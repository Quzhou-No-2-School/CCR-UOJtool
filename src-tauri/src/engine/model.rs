//! 核心数据模型：比赛 / 题目 / 测试点。
//!
//! 引擎内所有阶段（发现、解析、导出）共享这些类型。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 是否为安全的单段文件名（终审 I1）。
///
/// 拒绝：空值、含 `/` 或 `\`、含 `..` 子串、绝对路径（`Path::file_name` 不等于原值）。
/// `.prb` 测试点文件名与用户编辑的题号都必须经过此检查，
/// 防止 `dir.join("../x")` / `join("/abs")` 把目录外文件带进导出包。
pub fn is_safe_file_name(v: &str) -> bool {
    !v.is_empty()
        && !v.contains('/')
        && !v.contains('\\')
        && !v.contains("..")
        && Path::new(v).file_name() == Some(Path::new(v).as_os_str())
}

/// 自定义校验器可接受的源码扩展名（C/C++，与 CCR 编译源一致）。
pub const CHECKER_SOURCE_EXTS: &[&str] = &[".cpp", ".cc", ".cxx", ".c"];

/// 是否为校验器源码文件（终审 I2：排除 CCR 编译出的 ELF 二进制等产物）。
pub fn is_checker_source(file: &str) -> bool {
    CHECKER_SOURCE_EXTS.iter().any(|ext| file.ends_with(ext))
}

/// CCR 题目类型（对应 `.prb` 根节点 `type` 属性，见计划 0.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemKind {
    Traditional,
    AnswersOnly,
    Interactive,
    Unknown,
}

impl ProblemKind {
    /// 中文名（用于跳过警告等用户可见文案）。
    pub fn display_name(&self) -> &'static str {
        match self {
            ProblemKind::Traditional => "传统题",
            ProblemKind::AnswersOnly => "提交答案题",
            ProblemKind::Interactive => "交互题",
            ProblemKind::Unknown => "未知类型题",
        }
    }
}

/// 单个测试点（文件相对 `data/<题名>/`）。
#[derive(Debug, Clone, Serialize)]
pub struct TestCase {
    pub in_file: PathBuf,
    pub out_file: PathBuf,
    /// 时限（秒，可为小数）。
    pub time_limit: f64,
    /// 内存限制（MB）。
    pub memory_limit: f64,
    /// 所属 subtask 分值。
    pub score: u32,
}

/// 一道题的解析结果。
#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    /// 目录名（如 "A"）。
    pub name: String,
    pub kind: ProblemKind,
    /// `<task input>`，如 "A.in"。
    pub in_file: Option<String>,
    /// `<task output>`，如 "A.out"。
    pub out_file: Option<String>,
    /// 内置校验器名（去扩展名后，如 "fulltext"）。
    pub checker: Option<String>,
    /// data 目录中的自定义校验器文件名（若有）。
    pub custom_checker: Option<String>,
    /// 测试点（数字感知排序后）。
    pub cases: Vec<TestCase>,
    /// 解析阶段警告。
    pub warnings: Vec<String>,
    pub prb_found: bool,
}

impl Problem {
    /// 统一时限取最大值（决策 D3）。
    pub fn max_time(&self) -> f64 {
        self.cases
            .iter()
            .map(|c| c.time_limit)
            .fold(f64::MIN, f64::max)
    }

    /// 统一内存取最大值（决策 D3）。
    pub fn max_mem(&self) -> f64 {
        self.cases
            .iter()
            .map(|c| c.memory_limit)
            .fold(f64::MIN, f64::max)
    }

    /// subtask 分值是否均匀（所有 case score 相等即视为均匀）。
    pub fn uniform_score(&self) -> bool {
        match self.cases.first() {
            Some(first) => self.cases.iter().all(|c| c.score == first.score),
            None => true,
        }
    }
}

/// 一场比赛。
#[derive(Debug, Serialize)]
pub struct Contest {
    pub name: String,
    pub path: PathBuf,
    pub problems: Vec<Problem>,
}

/// 引擎统一错误类型。
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("zip 错误：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Other(String),
}

/// 单题导出结果（UOJ / Hydro 共用）。
#[derive(Debug, Serialize)]
pub struct ExportItem {
    /// 产出的 zip 路径；跳过时为 None。
    pub zip_path: Option<PathBuf>,
    pub skipped: bool,
    pub warnings: Vec<String>,
}

/// 每题用户编辑（IPC DTO，前端可只传部分题目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemEditDto {
    /// 对应题目目录名。
    pub name: String,
    /// 覆盖题号（UOJ input_pre / Hydro filename 与 zip 文件名）。
    pub target_id: Option<String>,
    /// 覆盖统一时限（秒）。
    pub time_limit: Option<f64>,
    /// 覆盖统一内存（MB）。
    pub memory_limit: Option<f64>,
    /// 覆盖 checker（UOJ use_builtin_checker / Hydro checker_type）。
    pub checker: Option<String>,
    /// 是否导出该题。
    pub enabled: bool,
}

/// 测试点 DTO（文件名为字符串）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseDto {
    pub in_file: String,
    pub out_file: String,
    pub time_limit: f64,
    pub memory_limit: f64,
    pub score: u32,
}

/// 题目 DTO（附带引擎推断的统一限制与均匀性，供前端做默认值）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemDto {
    pub name: String,
    pub kind: ProblemKind,
    pub in_file: Option<String>,
    pub out_file: Option<String>,
    pub checker: Option<String>,
    pub custom_checker: Option<String>,
    pub cases: Vec<TestCaseDto>,
    pub warnings: Vec<String>,
    pub prb_found: bool,
    /// 统一时限（秒，各点最大值）。
    pub max_time: f64,
    /// 统一内存（MB，各点最大值）。
    pub max_mem: f64,
    pub uniform_score: bool,
}

/// 比赛 DTO。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContestDto {
    pub name: String,
    pub path: String,
    pub problems: Vec<ProblemDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造带 n 个测试点的题目，score/time/mem 由切片逐点给出。
    fn problem_with(scores: &[u32], times: &[f64], mems: &[f64]) -> Problem {
        let cases = scores
            .iter()
            .enumerate()
            .map(|(i, &s)| TestCase {
                in_file: PathBuf::from(format!("P{}.in", i + 1)),
                out_file: PathBuf::from(format!("P{}.out", i + 1)),
                time_limit: times[i],
                memory_limit: mems[i],
                score: s,
            })
            .collect();
        Problem {
            name: "P".into(),
            kind: ProblemKind::Traditional,
            in_file: None,
            out_file: None,
            checker: None,
            custom_checker: None,
            cases,
            warnings: Vec::new(),
            prb_found: true,
        }
    }

    #[test]
    fn max_time_takes_largest() {
        let p = problem_with(&[10, 10, 10], &[0.5, 2.0, 1.0], &[128.0; 3]);
        assert_eq!(p.max_time(), 2.0);
    }

    #[test]
    fn max_mem_takes_largest() {
        let p = problem_with(&[10, 10, 10], &[1.0; 3], &[128.0, 512.0, 256.0]);
        assert_eq!(p.max_mem(), 512.0);
    }

    #[test]
    fn max_time_empty_cases_is_f64_min() {
        let p = problem_with(&[], &[], &[]);
        assert_eq!(p.max_time(), f64::MIN);
    }

    #[test]
    fn uniform_score_true_when_all_equal() {
        let p = problem_with(&[10, 10, 10], &[1.0; 3], &[128.0; 3]);
        assert!(p.uniform_score());
    }

    #[test]
    fn uniform_score_false_when_mixed() {
        let p = problem_with(&[30, 70], &[1.0; 2], &[128.0; 2]);
        assert!(!p.uniform_score());
    }

    #[test]
    fn uniform_score_true_when_empty() {
        let p = problem_with(&[], &[], &[]);
        assert!(p.uniform_score());
    }
}
