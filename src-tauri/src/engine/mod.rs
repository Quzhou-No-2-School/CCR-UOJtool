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

use model::{Contest, EngineError, Problem, ProblemEditDto, ProblemKind};

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
    todo!()
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
