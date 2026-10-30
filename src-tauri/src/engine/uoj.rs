//! UOJ 导出：`problem.conf` 生成与 checker 映射（计划 0.4 / 0.7）。

use crate::engine::model::Problem;

/// UOJ 导出选项（用户可在 UI 覆盖 checker）。
#[derive(Debug, Clone, Default)]
pub struct UojOptions {
    pub checker_override: Option<String>,
}

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

    // 统一限制取最大值（D3），各点不一致时警告。
    let (time_limit, mem_limit) = if p.cases.is_empty() {
        warnings.push("该题没有测试点，使用默认限制 1s/128MB".to_string());
        (1, 128)
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
        let t = max_time.ceil() as i64;
        if t as f64 != max_time {
            warnings.push(format!("时限上取整：{max_time}s → {t}s"));
        }
        let m = max_mem.ceil() as i64;
        if m as f64 != max_mem {
            warnings.push(format!("内存限制上取整：{max_mem}MB → {m}MB"));
        }
        (t, m)
    };

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
        name = p.name
    );
    (conf, warnings)
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
}
