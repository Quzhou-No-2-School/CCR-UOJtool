//! 比赛组装：读 `.ccr` 顺序、扫描 `data/` 目录、`.prb` 解析与 discover 对账、无配置降级（D6）。

use crate::engine::model::{Contest, Problem, ProblemKind, TestCase};
use crate::engine::prb::parse_prb;
use std::path::Path;

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
    todo!()
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        // 实际测试在 tests/contest_discovery.rs（集成测试，见计划 Task 6）。
    }
}
