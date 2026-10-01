use crate::{config, library, organize, smartcopy, thumbs, AppState};
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[tauri::command]
pub fn import_paths(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<library::ImportResult, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let library_root = state.library_root_clone();
    let p: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let result = library::import_paths(&conn, &library_root, &p).map_err(|e| e.to_string())?;
    drop(conn);
    // 导入顺带：新入库的 hash 排进缩略图队列（同 hash 去重，miss 才生成）
    if !result.content_hashes.is_empty() {
        thumbs::enqueue(&app, &state, &result.content_hashes);
    }
    Ok(result)
}

#[tauri::command]
pub fn list_memes(
    state: State<'_, AppState>,
    view: library::GalleryView,
    query: Option<String>,
) -> Result<Vec<library::Meme>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    // 非空查询走搜索（多关键词 AND，范围内限定），空查询等价于列全视图
    let mut memes = match query.as_deref() {
        Some(q) if !q.trim().is_empty() => {
            library::search_memes(&conn, &view, q).map_err(|e| e.to_string())?
        }
        _ => library::list_memes(&conn, &view).map_err(|e| e.to_string())?,
    };
    drop(conn);
    let library_root = state.library_root_clone();
    for m in memes.iter_mut() {
        m.internal_path = library_root.join(&m.internal_path).to_string_lossy().to_string();
        // 已缓存零成本：命中缓存才给路径，miss 由前端进入惰性生成队列
        m.thumbnail_path = thumbs::cached_thumbnail_path(&library_root, &m.content_hash);
    }
    Ok(memes)
}

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<library::Collection>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    library::list_collections(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_collection_groups(state: State<'_, AppState>) -> Result<Vec<library::CollectionGroup>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    library::list_collection_groups(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection_group(state: State<'_, AppState>, name: String) -> Result<library::CollectionGroup, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::create_collection_group(&conn, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_collection_group(state: State<'_, AppState>, id: i64, name: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::rename_collection_group(&conn, id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_collection_group(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::delete_collection_group(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_collection_groups(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::reorder_collection_groups(&conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn move_collection_to_group(state: State<'_, AppState>, id: i64, group_id: Option<i64>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::move_collection_to_group(&conn, id, group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_collection(state: State<'_, AppState>, name: String) -> Result<library::Collection, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::create_collection(&conn, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_collection(state: State<'_, AppState>, id: i64, name: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::rename_collection(&conn, id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::delete_collection(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_collections(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::reorder_collections(&conn, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<organize::Tag>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_tag(
    state: State<'_, AppState>,
    meme_id: i64,
    name: String,
) -> Result<Vec<organize::Tag>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::add_tag(&conn, meme_id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_tag(
    state: State<'_, AppState>,
    meme_id: i64,
    tag_id: i64,
) -> Result<Vec<organize::Tag>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::remove_tag(&conn, meme_id, tag_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, meme_id: i64, favorite: bool) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::set_favorite(&conn, meme_id, favorite).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_description(
    state: State<'_, AppState>,
    meme_id: i64,
    description: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::set_description(&conn, meme_id, &description).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_meme(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let library_root = state.library_root_clone();
    organize::delete_meme(&conn, &library_root, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn batch_edit_memes(app: tauri::AppHandle, meme_ids: Vec<i64>, action: organize::BatchAction) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        organize::batch_edit(&conn, &meme_ids, &action).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_memes(app: tauri::AppHandle, meme_ids: Vec<i64>) -> Result<organize::BatchDeleteResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        Ok(organize::batch_delete(&conn, &state.library_root_clone(), &meme_ids))
    }).await.map_err(|e| e.to_string())?
}

/// Smart Copy：静态图走位图通道；动图走临时副本文件引用 + 首帧位图兜底。
/// 成功后隐藏 Quick Picker 并把焦点还给呼出方应用。
#[tauri::command]
pub fn smart_copy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    meme_id: i64,
) -> Result<(), String> {
    // 1. 读库内原图
    let (bytes, ext, hash, filename) = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let library_root = state.library_root_clone();
        let (rel, ext, hash, filename) = library::find_meme_file(&conn, meme_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("表情不存在：{meme_id}"))?;
        let bytes = std::fs::read(library_root.join(rel)).map_err(|e| e.to_string())?;
        (bytes, ext, hash, filename)
    };

    // 2. 构造剪贴板格式集（Smart Copy 策略矩阵）
    let formats = build_clipboard_formats(&bytes, &ext, &hash, &filename, &state)?;

    // 3. 写剪贴板
    smartcopy::write_clipboard(&formats)?;

    // 4. 记录最近使用
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        crate::organize::touch_recent_at(&conn, meme_id, now).map_err(|e| e.to_string())?;
    }

    // 5. 隐藏 Picker，等隐藏生效后把焦点还给呼出方
    if let Some(picker) = app.get_webview_window("quick-picker") {
        let _ = picker.hide();
    }
    let focus = state.summoner_focus.lock().unwrap().take();
    let auto_paste = state.config.lock().map_err(|e| e.to_string())?.auto_paste;
    std::thread::sleep(std::time::Duration::from_millis(120));
    crate::focus::restore(focus.as_ref());
    if auto_paste {
        std::thread::sleep(std::time::Duration::from_millis(80));
        if !crate::focus::paste_if_editable(focus.as_ref()) {
            eprintln!("自动粘贴未执行：原输入目标不可确认或系统拒绝注入；图片已复制到剪贴板");
        }
    }
    Ok(())
}

/// 静态图：CF_DIB + CF_DIBV5 + PNG 流；
/// 动图：CF_HDROP（临时副本）+ 首帧 CF_DIB/CF_DIBV5/PNG 兜底。
fn build_clipboard_formats(
    bytes: &[u8],
    ext: &str,
    hash: &str,
    _filename: &str,
    state: &State<'_, AppState>,
) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let animated = thumbs::detect_animation(bytes).map_err(|e| e.to_string())?;
    if animated {
        // 临时副本：下次复制清上一批，再放当前这一份
        let temp_path = {
            let mut store = state.clipboard_temp.lock().map_err(|e| e.to_string())?;
            store.clear_previous().map_err(|e| e.to_string())?;
            // 保留扩展名让目标应用识别格式；文件名冲突会被覆盖（同批仅一份）
            let name = temp_copy_name(hash, ext);
            store.stage(bytes, &name).map_err(|e| e.to_string())?
        };
        let hdrop = smartcopy::hdrop_from_paths(&[temp_path.as_path()]);
        let frame = thumbs::decode_first_frame(bytes).map_err(|e| e.to_string())?;
        let rgba = frame.to_rgba8();
        Ok(vec![
            (smartcopy::CF_HDROP, hdrop),
            (smartcopy::CF_DIB, smartcopy::dib_from_rgba(rgba.as_raw(), rgba.width() as i32, rgba.height() as i32)),
            (smartcopy::CF_DIBV5, dibv5_from_rgba(rgba.as_raw(), rgba.width() as i32, rgba.height() as i32)),
            (png_format(), png_bytes(&frame)),
        ])
    } else {
        let frame = thumbs::decode_first_frame(bytes).map_err(|e| e.to_string())?;
        let rgba = frame.to_rgba8();
        Ok(vec![
            (smartcopy::CF_DIB, smartcopy::dib_from_rgba(rgba.as_raw(), rgba.width() as i32, rgba.height() as i32)),
            (smartcopy::CF_DIBV5, dibv5_from_rgba(rgba.as_raw(), rgba.width() as i32, rgba.height() as i32)),
            (png_format(), png_bytes(&frame)),
        ])
    }
}

fn temp_copy_name(hash: &str, ext: &str) -> String {
    format!("{}.{}", hash.chars().take(8).collect::<String>(), ext.to_ascii_lowercase())
}

#[cfg(test)]
mod paste_copy_tests {
    use super::*;

    #[test]
    fn animated_temp_copy_keeps_file_extension() {
        assert_eq!(temp_copy_name("0123456789abcdef", "GIF"), "01234567.gif");
    }
}

/// BITMAPV5HEADER（124 字节，BI_BITFIELDS + alpha 掩码）+ 自底向上 BGRA。
fn dibv5_from_rgba(rgba: &[u8], width: i32, height: i32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(124 + rgba.len());
    buf.extend_from_slice(&124u32.to_le_bytes()); // bV5Size
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&(height).to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // bV5Planes
    buf.extend_from_slice(&32u16.to_le_bytes()); // bV5BitCount
    buf.extend_from_slice(&3u32.to_le_bytes()); // bV5Compression = BI_BITFIELDS
    buf.extend_from_slice(&((width as u32) * 4 * (height as u32)).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5XPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5YPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5ClrUsed
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5ClrImportant
    buf.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // RedMask
    buf.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // GreenMask
    buf.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // BlueMask
    buf.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // AlphaMask
    buf.extend_from_slice(&0x5769_6E20u32.to_le_bytes()); // CSType = LCS_WINDOWS_COLOR_SPACE 'Win '
    buf.extend_from_slice(&[0u8; 36]); // CIEXYZTRIPLE endpoints
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5GammaRed
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5GammaGreen
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5GammaBlue
    buf.extend_from_slice(&4u32.to_le_bytes()); // bV5Intent = LCS_GM_IMAGES
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5ProfileData
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5ProfileSize
    buf.extend_from_slice(&0u32.to_le_bytes()); // bV5Reserved
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

/// "PNG" 注册格式的流字节：PNG 源直接用原字节，其余格式重编码为 PNG。
fn png_bytes(frame: &image::DynamicImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    frame
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap_or_default();
    out.into_inner()
}

#[cfg(windows)]
fn png_format() -> u32 {
    smartcopy::registered_format(windows::core::w!("PNG"))
        .unwrap_or(smartcopy::CF_DIB) // 注册失败时占用一个无害值，数据仍以 DIB 为准
}

#[cfg(not(windows))]
fn png_format() -> u32 {
    0
}

#[tauri::command]
pub fn request_thumbnail(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content_hash: String,
) -> Result<(), String> {
    thumbs::enqueue(&app, &state, std::slice::from_ref(&content_hash));
    Ok(())
}

// ---------------------------------------------------------------------------
// 票 08：设置、向导、托盘配套命令
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn collections_of_meme(
    state: State<'_, AppState>,
    meme_id: i64,
) -> Result<Vec<library::Collection>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::collections_of_meme(&conn, meme_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_meme_to_collection(
    state: State<'_, AppState>,
    meme_id: i64,
    collection_id: i64,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::add_meme_to_collection(&conn, meme_id, collection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_meme_from_collection(
    state: State<'_, AppState>,
    meme_id: i64,
    collection_id: i64,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    organize::remove_meme_from_collection(&conn, meme_id, collection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<config::AppConfig, String> {
    Ok(state.config.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub fn set_auto_paste(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.auto_paste = enabled;
    state.save_config(&cfg)
}

/// 向导第一步：切换（或确认）托管库位置。新库即时生效，路径写入配置。
#[tauri::command]
pub fn set_library_root(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let conn = library::open_db(&path).map_err(|e| e.to_string())?;
    *state.conn.lock().map_err(|e| e.to_string())? = conn;
    *state.library_root.lock().map_err(|e| e.to_string())? = path.clone();
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.library_root = Some(path.to_string_lossy().to_string());
    state.save_config(&cfg)
}

#[tauri::command]
pub fn set_theme(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    theme: String,
) -> Result<(), String> {
    if !["system", "light", "dark"].contains(&theme.as_str()) {
        return Err(format!("未知主题：{theme}"));
    }
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.theme = theme.clone();
    state.save_config(&cfg)?;
    let _ = app.emit("theme-changed", cfg.theme);
    Ok(())
}

/// 录制时的即时冲突探测：试注册 → 立即解绑。Ok = 可用。
#[tauri::command]
pub fn probe_hotkey(app: tauri::AppHandle, accelerator: String) -> Result<(), String> {
    let gs = app.global_shortcut();
    // 若探测的正是当前生效的快捷键，先解绑再测，避免误报冲突
    let _ = gs.unregister(accelerator.as_str());
    gs.register(accelerator.as_str())
        .map_err(|e| format!("快捷键 {accelerator} 不可用（可能被其他程序占用）：{e}"))?;
    let _ = gs.unregister(accelerator.as_str());
    Ok(())
}

/// 应用并持久化新快捷键。
#[tauri::command]
pub fn set_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    accelerator: String,
) -> Result<(), String> {
    crate::apply_hotkey(&app, &accelerator)?;
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.hotkey = accelerator;
    state.save_config(&cfg)
}

#[tauri::command]
pub fn set_autostart(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.autostart = enabled;
    state.save_config(&cfg)
}

/// 向导收尾：应用快捷键 + 主题，标记已完成（二次启动不再出现）。
#[tauri::command]
pub fn complete_onboarding(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
    theme: String,
) -> Result<(), String> {
    crate::apply_hotkey(&app, &hotkey)?;
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    cfg.hotkey = hotkey;
    cfg.theme = theme;
    cfg.onboarded = true;
    state.save_config(&cfg)
}

#[derive(Debug, serde::Serialize)]
pub struct StorageInfo {
    pub library_root: String,
    pub db_bytes: u64,
    pub cache_bytes: u64,
}

fn dir_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            total += dir_size(&p);
        } else if let Ok(m) = entry.metadata() {
            total += m.len();
        }
    }
    total
}

#[tauri::command]
pub fn get_storage_info(state: State<'_, AppState>) -> Result<StorageInfo, String> {
    let root = state.library_root_clone();
    let db_bytes = std::fs::metadata(root.join("library.db")).map(|m| m.len()).unwrap_or(0);
    let cache_bytes = dir_size(&thumbs::cache_dir(&root));
    Ok(StorageInfo {
        library_root: root.to_string_lossy().to_string(),
        db_bytes,
        cache_bytes,
    })
}

/// 一键清缓存：删除缩略图缓存目录内容（下次浏览按需重建）。
#[tauri::command]
pub fn clear_thumbnail_cache(state: State<'_, AppState>) -> Result<(), String> {
    let root = state.library_root_clone();
    smartcopy::TempStore::wipe_dir(&thumbs::cache_dir(&root)).map_err(|e| e.to_string())
}
