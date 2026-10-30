//! Hydro 导出：problem.yaml / config.yaml / 题面占位模板（计划 0.5 / 0.7）。
//!
//! yaml 用字符串模板手工拼接，不引入 serde_yaml，保持确定性输出顺序。

use crate::engine::model::{EngineError, ExportItem, Problem, ProblemKind};
use crate::engine::ziputil;
use std::path::Path;

/// Hydro 导出选项（用户可覆盖 checker_type / 题号 / 限制）。
#[derive(Debug, Clone, Default)]
pub struct HydroOptions {
    pub checker_override: Option<String>,
    /// 覆盖题号（filename 与 zip/文件夹名）。
    pub target_id: Option<String>,
    /// 覆盖统一时限（秒）。
    pub time_limit: Option<f64>,
    /// 覆盖统一内存（MB，非整数上取整）。
    pub memory_limit: Option<f64>,
}

/// 单题 Hydro 导出结果。
pub type HydroExportItem = ExportItem;

/// 文件 IO 前缀：`<task input>` 去扩展名（如 "A.in" → "A"），缺省回退题目录名（0.7 表）。
fn io_filename(p: &Problem) -> &str {
    p.in_file
        .as_deref()
        .and_then(|s| s.rsplit_once('.').map(|(stem, _)| stem))
        .unwrap_or(&p.name)
}

/// 生成 `testdata/config.yaml`（time 为毫秒整数，见计划 0.5 假设）。
pub fn build_config_yaml(p: &Problem, opts: &HydroOptions) -> (String, Vec<String>) {
    let mut warnings = Vec::new();

    // 统一限制取最大值（D3）；用户覆盖优先。
    let (time_ms, memory_mb) = if let (Some(t), Some(m)) = (opts.time_limit, opts.memory_limit) {
        ((t * 1000.0).round() as i64, m.ceil() as i64)
    } else if p.cases.is_empty() {
        warnings.push("该题没有测试点，使用默认限制 1000ms/128MB".to_string());
        (1000, 128)
    } else {
        let max_time = opts.time_limit.unwrap_or_else(|| p.max_time());
        let max_mem = opts.memory_limit.unwrap_or_else(|| p.max_mem());
        if opts.time_limit.is_none()
            && p.cases.iter().any(|c| c.time_limit != max_time)
        {
            warnings.push(format!("各测试点时限不一致，已统一取最大值 {max_time}s"));
        }
        if opts.memory_limit.is_none()
            && p.cases.iter().any(|c| c.memory_limit != max_mem)
        {
            warnings.push(format!(
                "各测试点内存限制不一致，已统一取最大值 {max_mem}MB"
            ));
        }
        // time 用毫秒整数可无损表达小数秒（0.5s → 500ms），无需上取整（0.5 假设）。
        let t = (max_time * 1000.0).round() as i64;
        let m = max_mem.ceil() as i64;
        if m as f64 != max_mem {
            warnings.push(format!("内存限制上取整：{max_mem}MB → {m}MB"));
        }
        (t, m)
    };

    // checker_type：用户覆盖 > 自定义校验器映射 ccr（0.7 表）> default。
    let (checker_type, checker_line) = match &opts.checker_override {
        Some(o) => (o.clone(), None),
        None => match &p.custom_checker {
            Some(file) => ("ccr".to_string(), Some(format!("checker: {file}\n"))),
            None => ("default".to_string(), None),
        },
    };

    let ty = match p.kind {
        ProblemKind::AnswersOnly => "submit_answer",
        ProblemKind::Interactive => "interactive",
        _ => "default",
    };

    let mut yaml = String::new();
    yaml.push_str(&format!("type: {ty}\n"));
    yaml.push_str(&format!("time: {time_ms}\n"));
    yaml.push_str(&format!("memory: {memory_mb}\n"));
    let filename = match &opts.target_id {
        Some(id) => id.clone(),
        None => io_filename(p).to_string(),
    };
    yaml.push_str(&format!("filename: {filename}\n"));
    yaml.push_str(&format!("checker_type: {checker_type}\n"));
    if let Some(line) = checker_line {
        yaml.push_str(&line);
    }
    yaml.push_str("cases:\n");
    for (i, _) in p.cases.iter().enumerate() {
        yaml.push_str(&format!("  - input: {}.in\n", i + 1));
        yaml.push_str(&format!("    output: {}.out\n", i + 1));
    }
    (yaml, warnings)
}

/// 生成 `problem.yaml`。
pub fn build_problem_yaml(p: &Problem) -> String {
    format!("title: {}\n", p.name)
}

/// 生成 `problem_zh.md` 占位题面（D10：CCR 数据不含题面）。
pub fn build_statement_md(p: &Problem) -> String {
    format!(
        "# {name}\n\n\
         题面待补充：CCR 比赛数据不含题面，请在 OJ 后台手动录入题目描述、\n\
         输入输出格式与样例。\n",
        name = p.name
    )
}

/// 把单题导出为 Hydro 包 zip：`out_dir/<题名>.zip`，内含一层题名文件夹
/// `<题名>/problem.yaml`、`<题名>/problem_zh.md`、`<题名>/testdata/config.yaml`
/// 与 `<题名>/testdata/<i>.in/.out`（0.5 结构）。非传统题按 D7 跳过。
pub fn export_problem_hydro(
    p: &Problem,
    contest_path: &Path,
    out_dir: &Path,
    opts: &HydroOptions,
) -> Result<HydroExportItem, EngineError> {
    // D7：提交答案题 / 交互题 V1 跳过。
    if p.kind != ProblemKind::Traditional {
        return Ok(ExportItem {
            zip_path: None,
            skipped: true,
            warnings: vec![format!("{}，V1 不支持转换，已跳过", p.kind.display_name())],
        });
    }

    let display_name = opts.target_id.as_deref().unwrap_or(&p.name).to_string();
    let (config_yaml, mut warnings) = build_config_yaml(p, opts);
    warnings.extend(p.warnings.iter().cloned());

    std::fs::create_dir_all(out_dir)?;
    let zip_path = out_dir.join(format!("{display_name}.zip"));
    let file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let prefix = format!("{display_name}/");

    ziputil::write_entry(&mut zip, &format!("{prefix}problem.yaml"), build_problem_yaml(p).as_bytes())?;
    ziputil::write_entry(&mut zip, &format!("{prefix}problem_zh.md"), build_statement_md(p).as_bytes())?;
    ziputil::write_entry(
        &mut zip,
        &format!("{prefix}testdata/config.yaml"),
        config_yaml.as_bytes(),
    )?;

    let data_dir = contest_path.join("data").join(&p.name);
    for (i, case) in p.cases.iter().enumerate() {
        let in_name = format!("{prefix}testdata/{}.in", i + 1);
        let out_name = format!("{prefix}testdata/{}.out", i + 1);
        ziputil::copy_file_into_zip(&mut zip, &in_name, &data_dir.join(&case.in_file))?;
        ziputil::copy_file_into_zip(&mut zip, &out_name, &data_dir.join(&case.out_file))?;
    }
    // 自定义校验器：有源码则一并复制进 testdata/（config.yaml 的 checker 指向它，0.7 表）。
    if let Some(file) = &p.custom_checker {
        ziputil::copy_file_into_zip(
            &mut zip,
            &format!("{prefix}testdata/{file}"),
            &data_dir.join(file),
        )?;
    }
    zip.finish()?;

    Ok(ExportItem {
        zip_path: Some(zip_path),
        skipped: false,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::model::{ProblemKind, TestCase};
    use std::path::PathBuf;

    fn problem_with(
        name: &str,
        times: &[f64],
        mems: &[f64],
        in_file: Option<&str>,
    ) -> Problem {
        let cases = times
            .iter()
            .enumerate()
            .map(|(i, &t)| TestCase {
                in_file: PathBuf::from(format!("{name}{}.in", i + 1)),
                out_file: PathBuf::from(format!("{name}{}.out", i + 1)),
                time_limit: t,
                memory_limit: mems[i],
                score: 10,
            })
            .collect();
        Problem {
            name: name.to_string(),
            kind: ProblemKind::Traditional,
            in_file: in_file.map(str::to_string),
            out_file: None,
            checker: Some("fulltext".to_string()),
            custom_checker: None,
            cases,
            warnings: Vec::new(),
            prb_found: true,
        }
    }

    #[test]
    fn config_yaml_exact_match_for_problem_a() {
        let a = problem_with("A", &[1.0; 10], &[128.0; 10], Some("A.in"));
        let (yaml, warnings) = build_config_yaml(&a, &HydroOptions::default());
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        let mut expected = String::from(
            "type: default\n\
             time: 1000\n\
             memory: 128\n\
             filename: A\n\
             checker_type: default\n\
             cases:\n",
        );
        for i in 1..=10 {
            expected.push_str(&format!("  - input: {i}.in\n    output: {i}.out\n"));
        }
        assert_eq!(yaml, expected);
    }

    #[test]
    fn problem_yaml_has_title() {
        let a = problem_with("A", &[1.0], &[128.0], Some("A.in"));
        let yaml = build_problem_yaml(&a);
        assert_eq!(yaml, "title: A\n");
    }

    #[test]
    fn statement_md_has_placeholder() {
        let a = problem_with("A", &[1.0], &[128.0], Some("A.in"));
        let md = build_statement_md(&a);
        assert!(md.contains("# A"));
        assert!(md.contains("题面待补充"));
    }

    #[test]
    fn b_problem_time_in_milliseconds() {
        let b = problem_with(
            "B",
            &[0.5, 1.0, 1.5, 2.0, 1.0],
            &[128.0, 256.0, 128.0, 512.0, 128.0],
            Some("B.in"),
        );
        let (yaml, warnings) = build_config_yaml(&b, &HydroOptions::default());
        assert!(yaml.starts_with("type: default\ntime: 2000\n"));
        assert!(yaml.contains("memory: 512\n"));
        assert!(warnings.iter().any(|w| w.contains("时限不一致")));
        assert!(warnings.iter().any(|w| w.contains("内存限制不一致")));
    }

    #[test]
    fn fractional_time_no_ceil_needed_in_ms() {
        let p = problem_with("P", &[0.5], &[128.0], None);
        let (yaml, warnings) = build_config_yaml(&p, &HydroOptions::default());
        assert!(yaml.contains("time: 500\n"));
        assert!(!warnings.iter().any(|w| w.contains("上取整")));
    }

    #[test]
    fn custom_checker_maps_to_ccr_type() {
        let mut c = problem_with("C", &[1.0, 1.0], &[128.0, 128.0], Some("C.in"));
        c.custom_checker = Some("checker.cpp".to_string());
        let (yaml, _) = build_config_yaml(&c, &HydroOptions::default());
        assert!(yaml.contains("checker_type: ccr\n"));
        assert!(yaml.contains("checker: checker.cpp\n"));

        // 无自定义校验器 → default，且不输出 checker 行
        let a = problem_with("A", &[1.0], &[128.0], Some("A.in"));
        let (yaml, _) = build_config_yaml(&a, &HydroOptions::default());
        assert!(yaml.contains("checker_type: default\n"));
        assert!(!yaml.contains("checker: "));
    }

    #[test]
    fn checker_override_replaces_type() {
        let a = problem_with("A", &[1.0], &[128.0], Some("A.in"));
        let opts = HydroOptions {
            checker_override: Some("testlib".to_string()),
            ..HydroOptions::default()
        };
        let (yaml, _) = build_config_yaml(&a, &opts);
        assert!(yaml.contains("checker_type: testlib\n"));
        assert!(!yaml.contains("checker: "));
    }

    #[test]
    fn filename_from_task_input_stem() {
        // <task input="FooBar.in"> → filename: FooBar
        let p = problem_with("X", &[1.0], &[128.0], Some("FooBar.in"));
        let (yaml, _) = build_config_yaml(&p, &HydroOptions::default());
        assert!(yaml.contains("filename: FooBar\n"));

        // 缺 <task input> → 回退题目录名
        let p = problem_with("X", &[1.0], &[128.0], None);
        let (yaml, _) = build_config_yaml(&p, &HydroOptions::default());
        assert!(yaml.contains("filename: X\n"));
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
                TestCase {
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
            kind: ProblemKind::Traditional,
            in_file: None,
            out_file: None,
            checker: Some("fulltext".to_string()),
            custom_checker: None,
            cases,
            warnings: Vec::new(),
            prb_found: true,
        };
        (problem, tmp.path().to_path_buf())
    }

    #[test]
    fn export_zip_with_problem_folder() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (a, contest_path) = fixture_on_disk(&tmp, "A", 3);
        let out_dir = tmp.path().join("out");

        let item = export_problem_hydro(&a, &contest_path, &out_dir, &HydroOptions::default())
            .unwrap();
        assert!(!item.skipped);
        let zip_path = item.zip_path.unwrap();
        assert_eq!(zip_path, out_dir.join("A.zip"));

        let f = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();

        // 恰有 9 个条目：yaml ×2 + config + 3×(in/out)，保留一层题名文件夹。
        let mut names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        names.sort();
        let mut expected = vec![
            "A/problem.yaml".to_string(),
            "A/problem_zh.md".to_string(),
            "A/testdata/config.yaml".to_string(),
        ];
        for i in 1..=3 {
            expected.push(format!("A/testdata/{i}.in"));
            expected.push(format!("A/testdata/{i}.out"));
        }
        expected.sort();
        assert_eq!(names, expected);

        // 内容与生成函数一致、数据字节一致。
        use std::io::Read;
        let mut s = String::new();
        archive
            .by_name("A/problem.yaml")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, build_problem_yaml(&a));

        s.clear();
        archive
            .by_name("A/problem_zh.md")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, build_statement_md(&a));

        s.clear();
        archive
            .by_name("A/testdata/config.yaml")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        let (want, _) = build_config_yaml(&a, &HydroOptions::default());
        assert_eq!(s, want);

        for i in 1..=3 {
            let mut data = String::new();
            archive
                .by_name(&format!("A/testdata/{i}.in"))
                .unwrap()
                .read_to_string(&mut data)
                .unwrap();
            assert_eq!(data, format!("A{i}.in\n"));
        }
    }

    #[test]
    fn answers_only_problem_is_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (mut d, contest_path) = fixture_on_disk(&tmp, "D", 2);
        d.kind = ProblemKind::AnswersOnly;
        let out_dir = tmp.path().join("out");

        let item = export_problem_hydro(&d, &contest_path, &out_dir, &HydroOptions::default())
            .unwrap();
        assert!(item.skipped);
        assert!(item.zip_path.is_none());
        assert!(!out_dir.join("D.zip").exists());
        assert!(item.warnings.iter().any(|w| w.contains("提交答案题")));
    }

    #[test]
    fn custom_checker_source_copied_into_testdata() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (mut c, contest_path) = fixture_on_disk(&tmp, "C", 2);
        c.custom_checker = Some("checker.cpp".to_string());
        // 真实的校验器源文件（会作为 testdata/checker.cpp 打进包）
        let src = contest_path.join("data/C/checker.cpp");
        std::fs::write(&src, "// checker.cpp source\n").unwrap();
        let out_dir = tmp.path().join("out");

        let item = export_problem_hydro(&c, &contest_path, &out_dir, &HydroOptions::default())
            .unwrap();
        assert!(!item.skipped);

        let f = std::fs::File::open(item.zip_path.unwrap()).unwrap();
        let mut archive = zip::ZipArchive::new(f).unwrap();
        let mut s = String::new();
        use std::io::Read;
        archive
            .by_name("C/testdata/checker.cpp")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, "// checker.cpp source\n");
    }

    /// 非源码校验器（如 CCR 编译出的 ELF 二进制）→ 不映射 ccr、不复制进包，
    /// 回退 default 并发警告（终审 I2）。
    #[test]
    fn binary_custom_checker_falls_back_to_default() {
        let mut c = problem_with("C", &[1.0, 1.0], &[128.0, 128.0], Some("C.in"));
        c.custom_checker = Some("checker".to_string());
        let (yaml, warnings) = build_config_yaml(&c, &HydroOptions::default());
        assert!(yaml.contains("checker_type: default\n"));
        assert!(!yaml.contains("checker: "));
        assert!(warnings.iter().any(|w| w.contains("非源码")));
    }

    /// 导出时非源码校验器文件不得被打进 testdata/。
    #[test]
    fn binary_custom_checker_not_copied_into_testdata() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (mut c, contest_path) = fixture_on_disk(&tmp, "C", 2);
        c.custom_checker = Some("checker".to_string());
        let src = contest_path.join("data/C/checker");
        std::fs::write(&src, b"\x7fELF\x02\x01\x01binary").unwrap();
        let out_dir = tmp.path().join("out");

        let item = export_problem_hydro(&c, &contest_path, &out_dir, &HydroOptions::default())
            .unwrap();
        assert!(!item.skipped);
        assert!(item.warnings.iter().any(|w| w.contains("非源码")));

        let f = std::fs::File::open(item.zip_path.unwrap()).unwrap();
        let archive = zip::ZipArchive::new(f).unwrap();
        let names: Vec<&str> = archive.file_names().collect();
        assert!(
            !names.contains(&"C/testdata/checker"),
            "二进制校验器不得进包，实际条目: {names:?}"
        );
        // 数据文件正常在包内
        assert!(names.contains(&"C/testdata/1.in"));
    }
}
