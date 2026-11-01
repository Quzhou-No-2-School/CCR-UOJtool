//! 转换引擎：解析 CCR-Plus 比赛并导出 UOJ / Hydro 题目包。

pub mod ccr;
pub mod discover;
pub mod hydro;
pub mod model;
pub mod prb;
pub mod report;
pub mod uoj;
pub mod ziputil;

pub use report::{ExportReport, ProblemReport};

use model::{Contest, EngineError, Problem, ProblemEditDto, ProblemKind, is_safe_file_name};

/// 导出目标格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Uoj,
    Hydro,
    Both,
}

/// 解析前端传入的 target 字符串（"uoj" / "hydro" / "both"）。
pub fn parse_target(s: &str) -> Option<Target> {
    match s {
        "uoj" => Some(Target::Uoj),
        "hydro" => Some(Target::Hydro),
        "both" => Some(Target::Both),
        _ => None,
    }
}

/// 导出选项：承载每题的用户编辑（未提及的题用引擎推断值）。
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub edits: Vec<ProblemEditDto>,
}

/// 编排整场导出：逐题按目标格式导出，收集报告；单题异常不阻塞其他题。
pub fn export_contest(
    contest: &Contest,
    target: Target,
    out_dir: &std::path::Path,
    opts: &ExportOptions,
) -> Result<ExportReport, EngineError> {
    std::fs::create_dir_all(out_dir)?;
    let mut items = Vec::new();

    for p in &contest.problems {
        let edit = opts.edits.iter().find(|e| e.name == p.name);

        // 用户禁用 → 跳过（不产 zip）。
        if edit.is_some_and(|e| !e.enabled) {
            let mut rep = base_report(p, "skipped");
            rep.skipped = true;
            rep.warnings.push("已被用户禁用，未导出".to_string());
            items.push(rep);
            continue;
        }

        // ANS / INT（D7）：跳过，只出现在报告中。
        if p.kind != ProblemKind::Traditional {
            let mut rep = base_report(p, "skipped");
            rep.skipped = true;
            rep.warnings
                .push(format!("{}，V1 不支持转换，已跳过", p.kind.display_name()));
            items.push(rep);
            continue;
        }

        let (uoj_opts, hydro_opts, sanitize_warnings) = build_target_options(edit, &p.name);

        if matches!(target, Target::Uoj | Target::Both) {
            let mut rep = base_report(p, "uoj");
            match uoj::export_problem_uoj(p, &contest.path, &out_dir.join("uoj"), &uoj_opts) {
                Ok(item) => {
                    rep.zip_path = item.zip_path.map(|z| z.display().to_string());
                    rep.skipped = item.skipped;
                    rep.warnings = item.warnings;
                }
                Err(e) => rep.errors.push(e.to_string()),
            }
            rep.warnings.extend(sanitize_warnings.iter().cloned());
            items.push(rep);
        }
        if matches!(target, Target::Hydro | Target::Both) {
            let mut rep = base_report(p, "hydro");
            match hydro::export_problem_hydro(p, &contest.path, &out_dir.join("hydro"), &hydro_opts)
            {
                Ok(item) => {
                    rep.zip_path = item.zip_path.map(|z| z.display().to_string());
                    rep.skipped = item.skipped;
                    rep.warnings = item.warnings;
                }
                Err(e) => rep.errors.push(e.to_string()),
            }
            rep.warnings.extend(sanitize_warnings.iter().cloned());
            items.push(rep);
        }
    }

    Ok(ExportReport { items })
}

/// 把用户编辑合并进两个目标的导出选项。
///
/// 终审 I1：target_id 会成为 zip 文件名 / 包内路径前缀，必须消毒——
/// 含路径分隔符、`..` 或为绝对路径时警告并回退题名（返回 None 即兜底）。
fn build_target_options(
    edit: Option<&ProblemEditDto>,
    fallback_name: &str,
) -> (uoj::UojOptions, hydro::HydroOptions, Vec<String>) {
    let edit = edit.filter(|e| e.enabled);
    let mut sanitize_warnings = Vec::new();
    let raw_target_id = edit
        .and_then(|e| e.target_id.clone())
        .filter(|s| !s.is_empty());
    let target_id = match raw_target_id {
        Some(id) if is_safe_file_name(&id) => Some(id),
        Some(id) => {
            sanitize_warnings.push(format!(
                "题号 {id:?} 含非法字符，已回退为题名 {fallback_name}"
            ));
            None
        }
        None => None,
    };
    let checker = edit
        .and_then(|e| e.checker.clone())
        .filter(|s| !s.is_empty());
    let time = edit.and_then(|e| e.time_limit);
    let mem = edit.and_then(|e| e.memory_limit);
    (
        uoj::UojOptions {
            checker_override: checker.clone(),
            target_id: target_id.clone(),
            time_limit: time,
            memory_limit: mem,
        },
        hydro::HydroOptions {
            checker_override: checker,
            target_id,
            time_limit: time,
            memory_limit: mem,
        },
        sanitize_warnings,
    )
}

/// 构造单题报告骨架。
fn base_report(p: &Problem, target: &str) -> ProblemReport {
    ProblemReport {
        problem: p.name.clone(),
        target: target.to_string(),
        zip_path: None,
        n_tests: p.cases.len(),
        skipped: false,
        warnings: Vec::new(),
        errors: Vec::new(),
    }
}
