//! UOJ 导出：`problem.conf` 生成与 checker 映射、扁平 zip 打包（计划 0.4 / 0.7）。

use crate::engine::model::{EngineError, ExportItem, Problem, ProblemKind};
use crate::engine::ziputil;
use std::path::Path;

/// UOJ 导出选项（用户可在 UI 覆盖 checker / 题号 / 限制）。
#[derive(Debug, Clone, Default)]
pub struct UojOptions {
    pub checker_override: Option<String>,
    /// 覆盖题号（input_pre/output_pre 与 zip 文件名）。
    pub target_id: Option<String>,
    /// 覆盖统一时限（秒，非整数会再次上取整）。
    pub time_limit: Option<f64>,
    /// 覆盖统一内存（MB，非整数上取整）。
    pub memory_limit: Option<f64>,
}

/// 单题 UOJ 导出结果。
pub type UojExportItem = ExportItem;

/// CCR 内置校验器 → UOJ 内建 checker。
/// 保守映射（计划 0.4 假设）：fulltext ≈ lcmp；未知值也映射 lcmp，由调用方附加警告。
pub fn map_checker(ccr_checker: &str) -> &'static str {
    let _ = ccr_checker;
    "lcmp"
}

/// 生成 UOJ `problem.conf` 文本（键顺序固定，见计划 0.4 样例）。
pub fn build_problem_conf(p: &Problem, opts: &UojOptions) -> (String, Vec<String>) {
    let mut warnings = Vec::new();

    // 自定义校验器：UOJ 判题接口不兼容，必须人工适配（0.7 表）。
    if let Some(file) = &p.custom_checker {
        warnings.push(format!(
            "自定义校验器 {file} 与 UOJ 判题接口不兼容，需人工改写为 testlib checker"
        ));
    }

    // checker：用户覆盖 > 保守映射。
    let checker = match &opts.checker_override {
        Some(o) => o.clone(),
        None => {
            let raw = p.checker.as_deref().unwrap_or("fulltext");
            let mapped = map_checker(raw);
            if raw != "fulltext" {
                warnings.push(format!(
                    "CCR 校验器 {raw} 非内置 fulltext，已保守映射为 {mapped}，请人工确认"
                ));
            }
            mapped.to_string()
        }
    };

    // 统一限制取最大值（D3），各点不一致时警告；用户覆盖优先。
    let (time_limit, mem_limit) = if let (Some(t), Some(m)) = (opts.time_limit, opts.memory_limit) {
        (
            ceil_warn(t, "时限", "s", &mut warnings),
            ceil_warn(m, "内存限制", "MB", &mut warnings),
        )
    } else if p.cases.is_empty() {
        warnings.push("该题没有测试点，使用默认限制 1s/128MB".to_string());
        (1, 128)
    } else {
        let max_time = opts.time_limit.unwrap_or_else(|| p.max_time());
        let max_mem = opts.memory_limit.unwrap_or_else(|| p.max_mem());
        if opts.time_limit.is_none() && p.cases.iter().any(|c| c.time_limit != max_time) {
            warnings.push(format!("各测试点时限不一致，已统一取最大值 {max_time}s"));
        }
        if opts.memory_limit.is_none() && p.cases.iter().any(|c| c.memory_limit != max_mem) {
            warnings.push(format!(
                "各测试点内存限制不一致，已统一取最大值 {max_mem}MB"
            ));
        }
        (
            ceil_warn(max_time, "时限", "s", &mut warnings),
            ceil_warn(max_mem, "内存限制", "MB", &mut warnings),
        )
    };

    let display_name = opts.target_id.as_deref().unwrap_or(&p.name);
    let n = p.cases.len();
    let conf = format!(
        "use_builtin_judger on\n\
         use_builtin_checker {checker}\n\
         n_tests {n}\n\
         n_ex_tests 0\n\
         n_sample_tests 0\n\
         input_pre {name}\n\
         input_suf in\n\
         output_pre {name}\n\
         output_suf out\n\
         time_limit {time_limit}\n\
         memory_limit {mem_limit}\n\
         output_limit 64\n",
        name = display_name
    );
    (conf, warnings)
}

/// 上取整为整数，非整数时附加警告。
fn ceil_warn(value: f64, label: &str, unit: &str, warnings: &mut Vec<String>) -> i64 {
    let v = value.ceil() as i64;
    if v as f64 != value {
        warnings.push(format!("{label}上取整：{value}{unit} → {v}{unit}"));
    }
    v
}

/// 把单题导出为 UOJ 扁平 zip：`out_dir/<题名>.zip`，根层级含 problem.conf 与
/// `<题名><i>.in/.out`（i 从 1，无子目录）。非传统题按 D7 跳过。
pub fn export_problem_uoj(
    p: &Problem,
    contest_path: &Path,
    out_dir: &Path,
    opts: &UojOptions,
) -> Result<UojExportItem, EngineError> {
    // D7：提交答案题 / 交互题 V1 跳过。
    if p.kind != ProblemKind::Traditional {
        return Ok(UojExportItem {
            zip_path: None,
            skipped: true,
            warnings: vec![format!("{}，V1 不支持转换，已跳过", p.kind.display_name())],
        });
    }

    let display_name = opts.target_id.as_deref().unwrap_or(&p.name).to_string();
    let (conf, mut warnings) = build_problem_conf(p, opts);
    warnings.extend(p.warnings.iter().cloned());

    std::fs::create_dir_all(out_dir)?;
    let zip_path = out_dir.join(format!("{display_name}.zip"));
    let file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);

    ziputil::write_entry(&mut zip, "problem.conf", conf.as_bytes())?;
    let data_dir = contest_path.join("data").join(&p.name);
    for (i, case) in p.cases.iter().enumerate() {
        let in_name = format!("{display_name}{}.in", i + 1);
        let out_name = format!("{display_name}{}.out", i + 1);
        ziputil::copy_file_into_zip(&mut zip, &in_name, &data_dir.join(&case.in_file))?;
        ziputil::copy_file_into_zip(&mut zip, &out_name, &data_dir.join(&case.out_file))?;
    }
    zip.finish()?;

    Ok(UojExportItem {
        zip_path: Some(zip_path),
        skipped: false,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn problem_with(name: &str, times: &[f64], mems: &[f64]) -> Problem {
        let cases = times
            .iter()
            .enumerate()
            .map(|(i, &t)| crate::engine::model::TestCase {
                in_file: PathBuf::from(format!("{name}{}.in", i + 1)),
                out_file: PathBuf::from(format!("{name}{}.out", i + 1)),
                time_limit: t,
                memory_limit: mems[i],
                score: 10,
            })
            .collect();
        Problem {
            name: name.to_string(),
            kind: crate::engine::model::ProblemKind::Traditional,
            in_file: None,
            out_file: None,
            checker: Some("fulltext".to_string()),
            custom_checker: None,
            cases,
            warnings: Vec::new(),
            prb_found: true,
        }
    }

    #[test]
    fn conf_exact_match_for_problem_a() {
        let a = problem_with("A", &[1.0; 10], &[128.0; 10]);
        let (conf, warnings) = build_problem_conf(&a, &UojOptions::default());
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(
            conf,
            "use_builtin_judger on\n\
             use_builtin_checker lcmp\n\
             n_tests 10\n\
             n_ex_tests 0\n\
             n_sample_tests 0\n\
             input_pre A\n\
             input_suf in\n\
             output_pre A\n\
             output_suf out\n\
             time_limit 1\n\
             memory_limit 128\n\
             output_limit 64\n"
        );
    }

    #[test]
    fn max_time_two_seconds_no_ceil_warning() {
        let b = problem_with(
            "B",
            &[0.5, 1.0, 1.5, 2.0, 1.0],
            &[128.0, 256.0, 128.0, 512.0, 128.0],
        );
        let (conf, warnings) = build_problem_conf(&b, &UojOptions::default());
        assert!(conf.contains("time_limit 2\n"));
        assert!(conf.contains("memory_limit 512\n"));
        assert!(!warnings.iter().any(|w| w.contains("时限上取整")));
        assert!(warnings.iter().any(|w| w.contains("时限不一致")));
        assert!(warnings.iter().any(|w| w.contains("内存限制不一致")));
    }

    #[test]
    fn fractional_time_ceil_with_warning() {
        let p = problem_with("P", &[0.5], &[128.0]);
        let (conf, warnings) = build_problem_conf(&p, &UojOptions::default());
        assert!(conf.contains("time_limit 1\n"));
        assert!(warnings.iter().any(|w| w.contains("时限上取整")));
    }

    #[test]
    fn fractional_mem_ceil_with_warning() {
        let p = problem_with("P", &[1.0], &[128.5]);
        let (conf, warnings) = build_problem_conf(&p, &UojOptions::default());
        assert!(conf.contains("memory_limit 129\n"));
        assert!(warnings.iter().any(|w| w.contains("内存")));
    }

    #[test]
    fn checker_override_used_verbatim() {
        let p = problem_with("P", &[1.0], &[128.0]);
        let opts = UojOptions {
            checker_override: Some("nonte xt".to_string()),
            ..UojOptions::default()
        };
        let (conf, warnings) = build_problem_conf(&p, &opts);
        assert!(conf.contains("use_builtin_checker nonte xt\n"));
        assert!(warnings.is_empty());
    }

    #[test]
    fn non_fulltext_checker_warns_but_maps_to_lcmp() {
        let mut p = problem_with("P", &[1.0], &[128.0]);
        p.checker = Some("ncmp".to_string());
        let (conf, warnings) = build_problem_conf(&p, &UojOptions::default());
        assert!(conf.contains("use_builtin_checker lcmp\n"));
        assert!(warnings.iter().any(|w| w.contains("lcmp")));
    }

    #[test]
    fn custom_checker_warns_manual_adaptation() {
        let mut p = problem_with("P", &[1.0], &[128.0]);
        p.custom_checker = Some("checker.cpp".to_string());
        let (_, warnings) = build_problem_conf(&p, &UojOptions::default());
        assert!(warnings.iter().any(|w| w.contains("自定义校验器")));
    }

    #[test]
    fn map_checker_fulltext_to_lcmp() {
        assert_eq!(map_checker("fulltext"), "lcmp");
        assert_eq!(map_checker("whatever"), "lcmp");
    }

    /// 构造含真实数据文件的题目目录：data/<name>/<name><i>.in/.out。
    fn fixture_on_disk(
        tmp: &tempfile::TempDir,
        name: &str,
        n: usize,
    ) -> (Problem, std::path::PathBuf) {
        let data_dir = tmp.path().join("data").join(name);
        std::fs::create_dir_all(&data_dir).unwrap();
        let cases = (1..=n)
            .map(|i| {
                let in_name = format!("{name}{i}.in");
                let out_name = format!("{name}{i}.out");
                std::fs::write(data_dir.join(&in_name), format!("{in_name}\n")).unwrap();
                std::fs::write(data_dir.join(&out_name), format!("{out_name}\n")).unwrap();
                crate::engine::model::TestCase {
                    in_file: PathBuf::from(&in_name),
                    out_file: PathBuf::from(&out_name),
                    time_limit: 1.0,
                    memory_limit: 128.0,
                    score: 10,
                }
            })
            .collect();
        let problem = Problem {
            name: name.to_string(),
            kind: crate::engine::model::ProblemKind::Traditional,
            in_file: None,
            out_file: None,
            checker: Some("fulltext".to_string()),
            custom_checker: None,
            cases,
            warnings: vec!["解析阶段警告样例".to_string()],
            prb_found: true,
        };
        (problem, tmp.path().to_path_buf())
    }

    #[test]
    fn export_flat_zip_with_21_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (a, contest_path) = fixture_on_disk(&tmp, "A", 10);
        let out_dir = tmp.path().join("out");

        let item = export_problem_uoj(&a, &contest_path, &out_dir, &UojOptions::default()).unwrap();
        assert!(!item.skipped);
        let zip_path = item.zip_path.unwrap();
        assert_eq!(zip_path, out_dir.join("A.zip"));

        let f = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();

        // 根层级恰有 problem.conf + 10×.in + 10×.out，无子目录。
        let mut names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        names.sort();
        let mut expected = vec!["problem.conf".to_string()];
        for i in 1..=10 {
            expected.push(format!("A{i}.in"));
        }
        for i in 1..=10 {
            expected.push(format!("A{i}.out"));
        }
        expected.sort();
        assert_eq!(names, expected);
        assert!(names.iter().all(|n| !n.contains('/')));

        // conf 内容与 Task 7 生成一致。
        let mut conf = String::new();
        use std::io::Read;
        archive
            .by_name("problem.conf")
            .unwrap()
            .read_to_string(&mut conf)
            .unwrap();
        let (want, _) = build_problem_conf(&a, &UojOptions::default());
        assert_eq!(conf, want);

        // 数据字节与源文件一致。
        for i in 1..=10 {
            let mut s = String::new();
            archive
                .by_name(&format!("A{i}.in"))
                .unwrap()
                .read_to_string(&mut s)
                .unwrap();
            assert_eq!(s, format!("A{i}.in\n"));
        }
    }

    #[test]
    fn export_carries_parse_warnings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (a, contest_path) = fixture_on_disk(&tmp, "A", 2);
        let out_dir = tmp.path().join("out");

        let item = export_problem_uoj(&a, &contest_path, &out_dir, &UojOptions::default()).unwrap();
        assert!(item.warnings.iter().any(|w| w.contains("解析阶段警告样例")));
    }

    #[test]
    fn answers_only_problem_is_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (mut d, contest_path) = fixture_on_disk(&tmp, "D", 2);
        d.kind = crate::engine::model::ProblemKind::AnswersOnly;
        let out_dir = tmp.path().join("out");

        let item = export_problem_uoj(&d, &contest_path, &out_dir, &UojOptions::default()).unwrap();
        assert!(item.skipped);
        assert!(item.zip_path.is_none());
        assert!(!out_dir.join("D.zip").exists());
        assert!(item.warnings.iter().any(|w| w.contains("提交答案题")));
    }
}
