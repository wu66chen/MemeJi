//! Smart Copy：剪贴板格式策略 + 临时文件生命周期。
//!
//! Seam：`TempStore` / `dib_from_rgba` / `hdrop_from_paths` / `write_clipboard` /
//! `copy_with_strategy`（仅 Windows；macOS 通道随打包票落地）。
//!
//! 策略（位图通道天然单帧，文件引用是唯一保动画通道）：
//! - 静态图：CF_HDROP 临时副本 + CF_DIB/CF_DIBV5/PNG，让不同聊天软件选择可接受格式
//! - 动图：CF_HDROP 指向临时副本 + 首帧 CF_DIB/CF_DIBV5/PNG 兜底只读位图的应用
//! - 临时文件：下次复制清上一批 + 应用退出全清（`TempStore`，单测覆盖）

use std::io;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 临时文件生命周期（跨平台、纯逻辑、可单测）
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct TempStore {
    dir: PathBuf,
    live: Vec<PathBuf>,
}

impl TempStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        TempStore {
            dir: dir.into(),
            live: Vec::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 放置新一批临时副本前，清掉上一批（「下次复制清上一批」）。
    pub fn clear_previous(&mut self) -> io::Result<()> {
        let mut failed = None;
        for path in self.live.drain(..) {
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    failed = failed.or(Some(e)); // 单个失败不中断，尽量清完
                }
            }
        }
        match failed {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// 写入一份临时副本，登记进当前批次。
    pub fn stage(&mut self, bytes: &[u8], name: &str) -> io::Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(name);
        std::fs::write(&path, bytes)?;
        self.live.push(path.clone());
        Ok(path)
    }

    /// 应用退出全清：清空目录内所有文件（含历史遗留），保留目录本身。
    pub fn wipe_all(&mut self) -> io::Result<()> {
        self.live.clear();
        Self::wipe_dir(&self.dir)
    }

    pub fn wipe_dir(dir: &Path) -> io::Result<()> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(()); // 目录不存在视为已清空
        };
        let mut failed = None;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Err(e) = std::fs::remove_dir_all(&p) {
                    failed = failed.or(Some(e));
                }
            } else if let Err(e) = std::fs::remove_file(&p) {
                failed = failed.or(Some(e));
            }
        }
        match failed {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Win32 剪贴板格式构造（纯字节构造，可单测）
// ---------------------------------------------------------------------------

/// 由 RGBA 构造 CF_DIB（BITMAPINFOHEADER + 自底向上 BGRA）。
pub fn dib_from_rgba(rgba: &[u8], width: i32, height: i32) -> Vec<u8> {
    assert_eq!(rgba.len(), (width as usize) * (height as usize) * 4);
    let mut buf = Vec::with_capacity(40 + rgba.len());
    // BITMAPINFOHEADER（40 字节）
    buf.extend_from_slice(&40u32.to_le_bytes()); // biSize
    buf.extend_from_slice(&width.to_le_bytes()); // biWidth
    buf.extend_from_slice(&(height).to_le_bytes()); // biHeight：正数 → 自底向上
    buf.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    buf.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    buf.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    buf.extend_from_slice(&((width as u32) * 4 * (height as u32)).to_le_bytes()); // biSizeImage
    buf.extend_from_slice(&0u32.to_le_bytes()); // biXPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biYPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
                                                // 像素：自底向上逐行，RGBA → BGRA
    let w = width as usize;
    for row in (0..height as usize).rev() {
        for px in rgba[row * w * 4..(row + 1) * w * 4].chunks_exact(4) {
            buf.push(px[2]);
            buf.push(px[1]);
            buf.push(px[0]);
            buf.push(px[3]);
        }
    }
    buf
}

/// 由路径列表构造 CF_HDROP（DROPFILES 头 + UTF-16 双空结尾）。
pub fn hdrop_from_paths(paths: &[&Path]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&20u32.to_le_bytes()); // pFiles：文件列表偏移
    buf.extend_from_slice(&[0u8; 12]); // pt(8) + fNC(4)
    buf.extend_from_slice(&1u32.to_le_bytes()); // fWide：UTF-16
    for path in paths {
        for unit in path.to_string_lossy().encode_utf16() {
            buf.extend_from_slice(&unit.to_le_bytes());
        }
        buf.extend_from_slice(&0u16.to_le_bytes()); // 单路径空结尾
    }
    buf.extend_from_slice(&0u16.to_le_bytes()); // 列表双空结尾
    buf
}

// ---------------------------------------------------------------------------
// Windows 剪贴板写入
// ---------------------------------------------------------------------------

pub const CF_DIB: u32 = 8;
pub const CF_HDROP: u32 = 15;
pub const CF_DIBV5: u32 = 17;

#[cfg(windows)]
use windows::core::PCWSTR;

/// 注册格式（如 "PNG"）——仅 Windows；失败返回 None，调用方降级省略该格式。
#[cfg(windows)]
pub fn registered_format(name: PCWSTR) -> Option<u32> {
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    let value = unsafe { RegisterClipboardFormatW(name) };
    (value != 0).then_some(value)
}

#[cfg(windows)]
fn set_clipboard_data(format: u32, data: &[u8]) -> Result<(), String> {
    use windows::Win32::Foundation::{GlobalFree, HANDLE};
    use windows::Win32::System::DataExchange::SetClipboardData;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    unsafe {
        let hglobal =
            GlobalAlloc(GMEM_MOVEABLE, data.len()).map_err(|e| format!("GlobalAlloc 失败: {e}"))?;
        let ptr = GlobalLock(hglobal);
        if ptr.is_null() {
            let _ = GlobalFree(Some(hglobal));
            return Err("GlobalLock 失败".into());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
        let _ = GlobalUnlock(hglobal);
        match SetClipboardData(format, Some(HANDLE(hglobal.0))) {
            Ok(_) => Ok(()), // 成功后剪贴板接管内存，不得 GlobalFree
            Err(e) => {
                let _ = GlobalFree(Some(hglobal));
                Err(format!("SetClipboardData 失败: {e}"))
            }
        }
    }
}

/// 打开 → 清空 → 逐格式写入 → 关闭。所有格式同写一个剪贴板条目。
#[cfg(windows)]
pub fn write_clipboard(formats: &[(u32, Vec<u8>)]) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err("打开剪贴板失败（被其他程序占用）".into());
        }
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| format!("EmptyClipboard 失败: {e}"))?;
            for (format, data) in formats {
                set_clipboard_data(*format, data)?;
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
pub fn write_clipboard(_formats: &[(u32, Vec<u8>)]) -> Result<(), String> {
    Err("当前平台的剪贴板通道尚未实现（macOS 随打包票落地）".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_store_next_copy_clears_previous_batch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = TempStore::new(tmp.path().join("clip"));

        let first = store.stage(b"first", "meme-aaaa.gif").unwrap();
        assert!(first.exists());

        // 下次复制：清上一批，再放新副本
        store.clear_previous().unwrap();
        assert!(!first.exists());
        let second = store.stage(b"second", "meme-bbbb.gif").unwrap();
        assert!(second.exists());

        let files: Vec<_> = std::fs::read_dir(tmp.path().join("clip"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files, vec!["meme-bbbb.gif"]);
    }

    #[test]
    fn temp_store_wipe_all_clears_dir_including_legacy_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("clip");
        let mut store = TempStore::new(dir.as_os_str());
        store.stage(b"a", "a.png").unwrap();
        // 历史遗留：不在 live 列表里的文件也该被清掉
        std::fs::write(dir.join("legacy.png"), b"legacy").unwrap();

        store.wipe_all().unwrap();

        let count = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(count, 0);
        assert!(dir.exists()); // 目录保留
    }

    #[test]
    fn wipe_dir_on_missing_directory_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(TempStore::wipe_dir(&tmp.path().join("不存在")).is_ok());
    }

    #[test]
    fn dib_builder_bottom_up_and_bgra() {
        // 1 宽 × 2 高：顶行红 [255,0,0,255]，底行绿 [0,255,0,255]
        let mut rgba = Vec::new();
        rgba.extend_from_slice(&[255, 0, 0, 255]); // 顶行
        rgba.extend_from_slice(&[0, 255, 0, 255]); // 底行
        let dib = dib_from_rgba(&rgba, 1, 2);

        assert_eq!(&dib[0..4], &40u32.to_le_bytes()); // biSize
        assert_eq!(&dib[4..8], &1i32.to_le_bytes()); // biWidth
        assert_eq!(&dib[8..12], &2i32.to_le_bytes()); // biHeight（正 → bottom-up）
        assert_eq!(&dib[12..14], &1u16.to_le_bytes()); // biPlanes
        assert_eq!(&dib[14..16], &32u16.to_le_bytes()); // biBitCount
                                                        // 第一像素是图像「最后一行」→ 绿色 BGRA
        assert_eq!(&dib[40..44], &[0u8, 255, 0, 255]);
        // 第二像素是顶行红色 BGRA
        assert_eq!(&dib[44..48], &[0u8, 0, 255, 255]);
    }

    #[test]
    fn hdrop_builder_uses_utf16_and_double_null() {
        let path = Path::new(r"C:\temp\meme.gif");
        let drop = hdrop_from_paths(&[path]);

        assert_eq!(&drop[0..4], &20u32.to_le_bytes()); // pFiles
        assert_eq!(&drop[16..20], &1u32.to_le_bytes()); // fWide
        let expected: Vec<u8> = path
            .to_string_lossy()
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .chain([0u8, 0]) // 路径空结尾
            .chain([0u8, 0]) // 列表双空结尾
            .collect();
        assert_eq!(&drop[20..], expected.as_slice());
    }
}
