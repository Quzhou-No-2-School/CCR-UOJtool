//! Tauri IPC 命令层：把引擎能力暴露给 Web 前端（参数/返回值全部 serde JSON）。

use crate::engine::ccr::{scan_contest as engine_scan_contest, ScanOptions};
use crate::engine::model::{Contest, ContestDto, ProblemDto, TestCaseDto};
use crate::engine::{export_contest, parse_target, ExportOptions, ExportReport};
use std::path::{Path, PathBuf};

/// 引擎 Contest → 前端 DTO（附带推断字段）。
pub fn contest_to_dto(contest: &Contest) -> ContestDto {
    ContestDto {
        name: contest.name.clone(),
        path: contest.path.display().to_string(),
        problems: contest
            .problems
            .iter()
            .map(|p| ProblemDto {
                name: p.name.clone(),
                kind: p.kind,
                in_file: p.in_file.clone(),
                out_file: p.out_file.clone(),
                checker: p.checker.clone(),
                custom_checker: p.custom_checker.clone(),
                cases: p
                    .cases
                    .iter()
                    .map(|c| TestCaseDto {
                        in_file: c.in_file.display().to_string(),
                        out_file: c.out_file.display().to_string(),
                        time_limit: c.time_limit,
                        memory_limit: c.memory_limit,
                        score: c.score,
                    })
                    .collect(),
                warnings: p.warnings.clone(),
                prb_found: p.prb_found,
                max_time: p.max_time(),
                max_mem: p.max_mem(),
                uniform_score: p.uniform_score(),
            })
            .collect(),
    }
}

/// 解压比赛 zip 到 `dest_root` 并定位比赛根目录。
/// zip 根层就是比赛（有 data/）时返回 dest_root；只有一个子目录时返回该子目录。
pub fn import_zip_to_dir(zip_path: &Path, dest_root: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("打开 {} 失败：{e}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 失败：{e}"))?;
    std::fs::create_dir_all(dest_root).map_err(|e| format!("创建 {} 失败：{e}", dest_root.display()))?;

    // enclosed_name 防 zip-slip：拒绝路径穿越条目。
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("读取 zip 条目失败：{e}"))?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("zip 内含不安全路径：{}", entry.name()));
        };
        let out_path = dest_root.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("创建 {} 失败：{e}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建 {} 失败：{e}", parent.display()))?;
            }
            let mut out = std::fs::File::create(&out_path)
                .map_err(|e| format!("创建 {} 失败：{e}", out_path.display()))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("解压 {} 失败：{e}", out_path.display()))?;
        }
    }

    // 根层即比赛（有 data/）→ 直接用；否则找唯一子目录。
    if dest_root.join("data").is_dir() {
        return Ok(dest_root.to_path_buf());
    }
    let subdirs: Vec<PathBuf> = std::fs::read_dir(dest_root)
        .map_err(|e| format!("读取 {} 失败：{e}", dest_root.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if let [only] = subdirs.as_slice() {
        return Ok(only.clone());
    }
    Err(format!(
        "{} 中未找到比赛目录（缺少 data/）",
        zip_path.display()
    ))
}

/// 打开 CCR-Plus 比赛目录，扫描出题目列表。
#[tauri::command]
pub fn scan_contest(path: String) -> Result<ContestDto, String> {
    let contest = engine_scan_contest(Path::new(&path), &ScanOptions::default())?;
    Ok(contest_to_dto(&contest))
}

/// 导入打包好的比赛 zip：解压到临时目录后扫描。
#[tauri::command]
pub fn import_contest_zip(zip_path: String) -> Result<ContestDto, String> {
    let src = Path::new(&zip_path);
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "contest".to_string());
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = std::env::temp_dir()
        .join("ccr-uojtool")
        .join(format!("{stem}-{}-{millis}", std::process::id()));
    let root = import_zip_to_dir(src, &dest)?;
    let contest = engine_scan_contest(&root, &ScanOptions::default())?;
    Ok(contest_to_dto(&contest))
}

/// 逐题导出为 UOJ / Hydro 数据包并返回报告。
#[tauri::command]
pub fn export_problems(
    contest_path: String,
    edits: Vec<crate::engine::model::ProblemEditDto>,
    target: String,
    out_dir: String,
) -> Result<ExportReport, String> {
    let target =
        parse_target(&target).ok_or_else(|| format!("未知导出目标 {target:?}（应为 uoj/hydro/both）"))?;
    let contest = engine_scan_contest(Path::new(&contest_path), &ScanOptions::default())?;
    export_contest(
        &contest,
        target,
        Path::new(&out_dir),
        &ExportOptions { edits },
    )
    .map_err(|e| e.to_string())
}
