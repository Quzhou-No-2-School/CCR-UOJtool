//! 端到端测试：扫描夹具比赛 → 双格式导出 → 报告与 zip 内容校验（计划 Task 11）。

mod common;

use ccr_uojtool_lib::engine::ccr::{ScanOptions, scan_contest};
use ccr_uojtool_lib::engine::model::ProblemEditDto;
use ccr_uojtool_lib::engine::{ExportOptions, Target, export_contest};
use common::build_fixture_contest;
use std::io::Read;
use tempfile::TempDir;

#[test]
fn e2e_export_both_targets() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    let contest = scan_contest(tmp.path(), &ScanOptions::default()).unwrap();
    let out = tmp.path().join("export");

    let report = export_contest(&contest, Target::Both, &out, &ExportOptions::default()).unwrap();

    // 输出目录：uoj/ 与 hydro/ 子目录各含 A/B/C 三个 zip；D 不产 zip。
    for sub in ["uoj", "hydro"] {
        for name in ["A", "B", "C"] {
            assert!(
                out.join(sub).join(format!("{name}.zip")).is_file(),
                "{sub}/{name}.zip 应存在"
            );
        }
        assert!(!out.join(sub).join("D.zip").exists());
    }

    // 报告：A/B/C 各 2 项（uoj+hydro），D 1 项 skipped。
    assert_eq!(report.items.len(), 7);
    let d: Vec<_> = report.items.iter().filter(|i| i.problem == "D").collect();
    assert_eq!(d.len(), 1);
    assert!(d[0].skipped);
    assert!(d[0].warnings.iter().any(|w| w.contains("提交答案题")));

    // C 题警告：UOJ 侧含非均匀分值 + 自定义校验器；Hydro 侧含非均匀分值。
    let cu = report
        .items
        .iter()
        .find(|i| i.problem == "C" && i.target == "uoj")
        .unwrap();
    assert!(cu.warnings.iter().any(|w| w.contains("非均匀子任务分值")));
    assert!(cu.warnings.iter().any(|w| w.contains("自定义校验器")));
    let ch = report
        .items
        .iter()
        .find(|i| i.problem == "C" && i.target == "hydro")
        .unwrap();
    assert!(ch.warnings.iter().any(|w| w.contains("非均匀子任务分值")));

    // B 题混合扩展名成功导出，n_tests=5。
    let bu = report
        .items
        .iter()
        .find(|i| i.problem == "B" && i.target == "uoj")
        .unwrap();
    assert_eq!(bu.n_tests, 5);
    assert!(!bu.skipped);

    // A 的 UOJ zip：根层级 21 条目，conf 正确。
    let f = std::fs::File::open(out.join("uoj/A.zip")).unwrap();
    let mut ar = zip::ZipArchive::new(f).unwrap();
    assert_eq!(ar.len(), 21);
    let mut conf = String::new();
    ar.by_name("problem.conf")
        .unwrap()
        .read_to_string(&mut conf)
        .unwrap();
    assert!(conf.contains("use_builtin_checker lcmp"));
    assert!(conf.contains("n_tests 10"));
    assert!(conf.contains("input_pre A"));
    assert!(conf.contains("time_limit 1"));

    // A 的 Hydro zip：23 条目（yaml×2 + config + 10×in/out），config 正确。
    let f = std::fs::File::open(out.join("hydro/A.zip")).unwrap();
    let mut ar = zip::ZipArchive::new(f).unwrap();
    assert_eq!(ar.len(), 23);
    let mut cfg = String::new();
    ar.by_name("A/testdata/config.yaml")
        .unwrap()
        .read_to_string(&mut cfg)
        .unwrap();
    assert!(cfg.contains("time: 1000"));
    assert!(cfg.contains("filename: A"));
    assert!(cfg.contains("checker_type: default"));
}

#[test]
fn e2e_edits_rename_disable_checker() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    let contest = scan_contest(tmp.path(), &ScanOptions::default()).unwrap();
    let out = tmp.path().join("export");

    let edits = vec![
        ProblemEditDto {
            name: "A".into(),
            target_id: Some("prog".into()),
            time_limit: None,
            memory_limit: None,
            checker: Some("wcmp".into()),
            enabled: true,
        },
        ProblemEditDto {
            name: "B".into(),
            target_id: None,
            time_limit: None,
            memory_limit: None,
            checker: None,
            enabled: false,
        },
    ];
    let report = export_contest(&contest, Target::Uoj, &out, &ExportOptions { edits }).unwrap();

    // A 按新题号 prog 命名；B 被禁用不产 zip。
    assert!(out.join("uoj/prog.zip").is_file());
    assert!(!out.join("uoj/A.zip").exists());
    assert!(!out.join("uoj/B.zip").exists());

    let b = report.items.iter().find(|i| i.problem == "B").unwrap();
    assert!(b.skipped);
    assert!(b.warnings.iter().any(|w| w.contains("禁用")));

    // prog.zip 的 conf 反映 target_id 与 checker 覆盖。
    let f = std::fs::File::open(out.join("uoj/prog.zip")).unwrap();
    let mut ar = zip::ZipArchive::new(f).unwrap();
    let mut conf = String::new();
    ar.by_name("problem.conf")
        .unwrap()
        .read_to_string(&mut conf)
        .unwrap();
    assert!(conf.contains("use_builtin_checker wcmp"));
    assert!(conf.contains("input_pre prog"));
    assert!(conf.contains("n_tests 10"));
}

/// target_id 含路径分隔符/穿越 → 拒绝并回退题名，警告提示（终审 I1）。
#[test]
fn e2e_target_id_sanitized() {
    let tmp = TempDir::new().unwrap();
    build_fixture_contest(tmp.path());
    let contest = scan_contest(tmp.path(), &ScanOptions::default()).unwrap();
    let out = tmp.path().join("export");

    let edits = vec![ProblemEditDto {
        name: "A".into(),
        target_id: Some("../evil".into()),
        time_limit: None,
        memory_limit: None,
        checker: None,
        enabled: true,
    }];
    let report = export_contest(&contest, Target::Uoj, &out, &ExportOptions { edits }).unwrap();

    // 兜底：zip 用原题名 A 命名，且不产生逃逸路径的文件
    assert!(out.join("uoj/A.zip").is_file());
    assert!(!out.join("evil.zip").exists());
    assert!(!tmp.path().join("evil.zip").exists());

    let a = report.items.iter().find(|i| i.problem == "A").unwrap();
    assert!(a.warnings.iter().any(|w| w.contains("题号")));

    // conf 内 input_pre 回退为题名 A
    let f = std::fs::File::open(out.join("uoj/A.zip")).unwrap();
    let mut ar = zip::ZipArchive::new(f).unwrap();
    let mut conf = String::new();
    ar.by_name("problem.conf")
        .unwrap()
        .read_to_string(&mut conf)
        .unwrap();
    assert!(conf.contains("input_pre A"));
}
