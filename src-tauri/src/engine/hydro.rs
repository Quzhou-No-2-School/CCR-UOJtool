//! Hydro 导出：problem.yaml / config.yaml / 题面占位模板（计划 0.5 / 0.7）。
//!
//! yaml 用字符串模板手工拼接，不引入 serde_yaml，保持确定性输出顺序。

use crate::engine::model::{Problem, ProblemKind};

/// Hydro 导出选项（用户可覆盖 checker_type）。
#[derive(Debug, Clone, Default)]
pub struct HydroOptions {
    pub checker_override: Option<String>,
}

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

    // 统一限制取最大值（D3）。
    let (time_ms, memory_mb) = if p.cases.is_empty() {
        warnings.push("该题没有测试点，使用默认限制 1000ms/128MB".to_string());
        (1000, 128)
    } else {
        let max_time = p.max_time();
        let max_mem = p.max_mem();
        if p.cases.iter().any(|c| c.time_limit != max_time) {
            warnings.push(format!("各测试点时限不一致，已统一取最大值 {max_time}s"));
        }
        if p.cases.iter().any(|c| c.memory_limit != max_mem) {
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
    yaml.push_str(&format!("filename: {}\n", io_filename(p)));
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
}
