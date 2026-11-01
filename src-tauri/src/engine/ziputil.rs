//! 共用 zip 写入助手（UOJ / Hydro 导出共用）。

use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use zip::ZipWriter;
use zip::result::ZipResult;
use zip::write::SimpleFileOptions;

/// 把内存字节作为一个条目写入 zip。
pub fn write_entry<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
) -> ZipResult<()> {
    zip.start_file(name, SimpleFileOptions::default())?;
    zip.write_all(bytes)?;
    Ok(())
}

/// 流式把磁盘文件拷入 zip（数据文件可能很大，不整体读入内存）。
pub fn copy_file_into_zip<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    path: &Path,
) -> ZipResult<()> {
    let mut f = File::open(path)?;
    zip.start_file(name, SimpleFileOptions::default())?;
    std::io::copy(&mut f, zip)?;
    Ok(())
}
