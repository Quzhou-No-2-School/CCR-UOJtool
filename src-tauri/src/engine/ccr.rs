//! 比赛组装：读 `.ccr` 顺序、扫描 `data/` 目录、`.prb` 解析与 discover 对账、无配置降级（D6）。

use crate::engine::discover::discover_cases;
use crate::engine::model::{Contest, Problem, ProblemKind, TestCase};
use crate::engine::prb::{parse_prb, strip_extension};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 扫描选项：缺省时限（秒）与内存（MB），用于降级与 `.prb` 缺省字段回填。
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    pub default_time: f64,
    pub default_mem: f64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            default_time: 1.0,
            default_mem: 128.0,
        }
    }
}

/// 扫描 CCR-Plus 比赛目录，组装出比赛（题目顺序与 `.ccr` 一致，未列入者字母序殿后）。
pub fn scan_contest(path: &Path, opts: &ScanOptions) -> Result<Contest, String> {
    let data_dir = path.join("data");
    if !data_dir.is_dir() {
        return Err(format!("{} 缺少 data 目录，不是 CCR-Plus 比赛目录", path.display()));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    // 1. 读 `.ccr`（XML `<contest><order><problem>名</problem>`）得题目顺序；缺失或损坏则忽略。
    let order = std::fs::read_to_string(path.join(".ccr"))
        .ok()
        .and_then(|text| parse_ccr_order(&text).ok())
        .unwrap_or_default();

    // 2. 枚举 data/ 下的题目目录（字母序，作为稳定基序）。
    let mut dir_names: Vec<String> = std::fs::read_dir(&data_dir)
        .map_err(|e| format!("读取 {} 失败：{e}", data_dir.display()))?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    dir_names.sort();

    // 3. 组装每道题：有 `.prb` 走配置解析，否则按传统题降级（D6）。
    let mut problems = Vec::new();
    for dir_name in &dir_names {
        let dir = data_dir.join(dir_name);
        let prb_path = dir.join(".prb");
        if prb_path.is_file() {
            problems.push(build_from_prb(dir_name, &dir, opts));
        } else {
            problems.push(build_degraded(dir_name, &dir, opts));
        }
    }

    // 4. 排序：`.ccr` 顺序优先，未列入者排后面（稳定排序保持字母序）。
    problems.sort_by_key(|p| order.iter().position(|n| n == &p.name).unwrap_or(usize::MAX));

    Ok(Contest {
        name,
        path: path.to_path_buf(),
        problems,
    })
}

/// 解析 `.ccr` 的 `<order>` 列表。
fn parse_ccr_order(xml: &str) -> Result<Vec<String>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML 语法错误：{e}"))?;
    let root = doc.root_element();
    let mut order = Vec::new();
    for order_el in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "order")
    {
        for p in order_el
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "problem")
        {
            if let Some(t) = p.text() {
                let t = t.trim();
                if !t.is_empty() {
                    order.push(t.to_string());
                }
            }
        }
    }
    Ok(order)
}

/// 有 `.prb` 的题目：解析配置、校验测试点文件、与 discover 结果对账。
/// `.prb` 损坏时按降级处理并附带错误信息（单题异常不阻塞整个扫描）。
fn build_from_prb(name: &str, dir: &Path, opts: &ScanOptions) -> Problem {
    let prb_path = dir.join(".prb");
    let mut warnings = Vec::new();

    match parse_prb(&prb_path) {
        Ok(cfg) => {
            let (pairs, discover_warnings) = discover_cases(dir);
            warnings.extend(discover_warnings);

            let custom_checker = find_custom_checker(dir, cfg.checker.as_deref());

            let mut cases = Vec::new();
            let mut listed: HashSet<String> = HashSet::new();
            for (score, points) in &cfg.subtasks {
                for pt in points {
                    listed.insert(pt.in_file.clone());
                    listed.insert(pt.out_file.clone());
                    if !dir.join(&pt.in_file).is_file() || !dir.join(&pt.out_file).is_file() {
                        warnings.push(format!("测试点 {} 文件缺失，已跳过", pt.in_file));
                        continue;
                    }
                    cases.push(TestCase {
                        in_file: PathBuf::from(&pt.in_file),
                        out_file: PathBuf::from(&pt.out_file),
                        time_limit: pt.time.unwrap_or(opts.default_time),
                        memory_limit: pt.mem.unwrap_or(opts.default_mem),
                        score: *score,
                    });
                }
            }
            // discover 配对成功但 `.prb` 未列出的文件 → 警告（不加入用例）。
            for (in_file, _) in &pairs {
                let fname = in_file.file_name().unwrap_or_default().to_string_lossy();
                if !listed.contains(fname.as_ref()) {
                    warnings.push(format!("目录中存在未列入 .prb 的测试文件 {fname}，已忽略"));
                }
            }

            let mut problem = Problem {
                name: name.to_string(),
                kind: cfg.kind,
                in_file: cfg.input,
                out_file: cfg.output,
                checker: cfg.checker,
                custom_checker,
                cases,
                warnings,
                prb_found: true,
            };
            warn_if_uneven(&mut problem);
            problem
        }
        Err(e) => {
            warnings.push(format!(
                "解析 .prb 失败（{e}），已按传统题降级解析该目录"
            ));
            let mut problem = build_degraded(name, dir, opts);
            problem.warnings.splice(0..0, warnings);
            problem
        }
    }
}

/// 无 `.prb` 的题目目录：按传统题降级（决策 D6）。
/// 测试点由 discover 推断，限制用默认值，分值按 CCR 空配置规则等分（余数补到最后一点）。
fn build_degraded(name: &str, dir: &Path, opts: &ScanOptions) -> Problem {
    let mut warnings = vec![format!(
        "目录 {name} 缺少 .prb，未在 CCR 中配置过，已按传统题降级解析"
    )];
    let (pairs, discover_warnings) = discover_cases(dir);
    warnings.extend(discover_warnings);

    let n = pairs.len();
    let base = if n > 0 { 100 / n } else { 0 };
    let mut scores = vec![base; n];
    let mut rem = if n > 0 { 100 - base * n } else { 0 };
    let mut i = n;
    while rem > 0 && i > 0 {
        i -= 1;
        scores[i] += 1;
        rem -= 1;
    }

    let cases = pairs
        .iter()
        .enumerate()
        .map(|(k, (in_path, out_path))| TestCase {
            in_file: in_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default(),
            out_file: out_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default(),
            time_limit: opts.default_time,
            memory_limit: opts.default_mem,
            score: scores[k] as u32,
        })
        .collect();

    let mut problem = Problem {
        name: name.to_string(),
        kind: ProblemKind::Traditional,
        in_file: None,
        out_file: None,
        checker: Some("fulltext".to_string()),
        custom_checker: find_custom_checker_source(dir),
        cases,
        warnings,
        prb_found: false,
    };
    warn_if_uneven(&mut problem);
    problem
}

/// subtask 分值非均匀时追加警告（决策 D8：UOJ 只能等分）。
fn warn_if_uneven(problem: &mut Problem) {
    if !problem.uniform_score() {
        problem
            .warnings
            .push("非均匀子任务分值：UOJ 导出将按 100/N 等分".to_string());
    }
}

/// 在题目目录中找 prb checker 对应的自定义校验器源文件（文件名主干与 checker 名一致）。
fn find_custom_checker(dir: &Path, checker: Option<&str>) -> Option<String> {
    let checker = checker?;
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n != ".prb" && strip_extension(n) == checker)
                .unwrap_or(false)
        })
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// 降级目录中按常见命名找自定义校验器源文件（checker/val + C/C++ 扩展名）。
fn find_custom_checker_source(dir: &Path) -> Option<String> {
    const STEMS: &[&str] = &["checker", "val"];
    const EXTS: &[&str] = &[".cpp", ".cc", ".cxx", ".c"];
    for stem in STEMS {
        for ext in EXTS {
            let candidate = format!("{stem}{ext}");
            if dir.join(&candidate).is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
