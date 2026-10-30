//! 导出报告类型（serde 序列化，供前端直接消费）。

use serde::Serialize;

/// 单题导出报告。
#[derive(Debug, Serialize)]
pub struct ProblemReport {
    /// 题目目录名。
    pub problem: String,
    /// 目标格式："uoj" / "hydro"；未导出时为 "skipped"。
    pub target: String,
    /// 产出的 zip 绝对/相对路径；跳过或失败时为 None。
    pub zip_path: Option<String>,
    pub n_tests: usize,
    pub skipped: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// 整场导出报告。
#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub items: Vec<ProblemReport>,
}
