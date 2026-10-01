mod commands;
pub mod config;
pub mod focus;
pub mod library;
pub mod official;
pub mod organize;
pub mod picker;
pub mod smartcopy;
pub mod thumbs;
pub mod updater;

use library::open_db;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub conn: Mutex<rusqlite::Connection>,
    /// 托管库根目录（向导里可改，改后立即换库）
    pub library_root: Mutex<PathBuf>,
    /// 进行中/排队中的缩略图生成（按 content hash 去重）
    pub thumb_queue: Mutex<HashSet<String>>,
    /// 剪贴板临时副本库（下次复制清上一批 + 退出全清）
    pub clipboard_temp: Mutex<smartcopy::TempStore>,
    /// 呼出 Quick Picker 前的前台应用（复制后恢复焦点）
    pub summoner_focus: Mutex<Option<focus::CapturedFocus>>,
    /// 应用偏好（与图库数据库分离存放）
    pub config: Mutex<config::AppConfig>,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn library_root_clone(&self) -> PathBuf {
        self.library_root.lock().unwrap().clone()
    }

    pub fn save_config(&self, config: &config::AppConfig) -> Result<(), String> {
        config::save(&self.config_path, config).map_err(|e| e.to_string())?;
        *self.config.lock().unwrap() = config.clone();
        Ok(())
    }
}

fn default_library_root() -> PathBuf {
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join("Pictures").join("MemeLibrary")
}

/// （重新）注册全局快捷键：先解绑旧的，再绑新的。
pub fn apply_hotkey(app: &tauri::AppHandle, accelerator: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| format!("解绑旧快捷键失败: {e}"))?;
    gs.register(accelerator)
        .map_err(|e| format!("快捷键 {accelerator} 注册失败（可能被其他程序占用）: {e}"))
}

fn toggle_quick_picker(app: &tauri::AppHandle) {
    let Some(picker) = app.get_webview_window("quick-picker") else {
        return;
    };
    if picker.is_visible().unwrap_or(false) {
        let _ = picker.hide();
    } else {
        // 呼出前记录前台应用（呼出方），复制成功后恢复焦点
        {
            let state = app.state::<AppState>();
            *state.summoner_focus.lock().unwrap() = focus::capture();
        }
        position_picker(&picker);
        let _ = picker.show();
        let _ = picker.set_focus();
        // 呼出重置：前端监听后回到「最近使用 + 搜索聚焦 + 首项高亮」
        let _ = app.emit_to("quick-picker", "picker-shown", ());
    }
}

fn position_picker(picker: &WebviewWindow) {
    let Ok(cursor) = picker.cursor_position() else {
        return;
    };
    let Ok(size) = picker.outer_size() else {
        return;
    };
    let Some(monitor) = picker.monitor_from_point(cursor.x, cursor.y).ok().flatten() else {
        return;
    };

    let m_pos = monitor.position();
    let m_size = monitor.size();
    let (x, y) = picker::compute_anchor_position(
        (cursor.x, cursor.y),
        picker::Rect {
            x: m_pos.x as f64,
            y: m_pos.y as f64,
            width: m_size.width as f64,
            height: m_size.height as f64,
        },
        (size.width as f64, size.height as f64),
        picker::PICKER_MARGIN,
    );
    let _ = picker.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[tauri::command]
fn hide_picker(app: tauri::AppHandle) {
    if let Some(picker) = app.get_webview_window("quick-picker") {
        let _ = picker.hide();
    }
}

#[tauri::command]
fn toggle_picker(app: tauri::AppHandle) {
    toggle_quick_picker(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例：必须最先注册。第二实例启动时立即退出，并把主窗口带回前台，
        // 避免托盘图标翻倍与多实例争抢同一 SQLite 库。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_quick_picker(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            hide_picker,
            toggle_picker,
            commands::import_paths,
            commands::list_memes,
            commands::list_collections,
            commands::list_collection_groups,
            commands::create_collection_group,
            commands::rename_collection_group,
            commands::delete_collection_group,
            commands::reorder_collection_groups,
            commands::move_collection_to_group,
            commands::request_thumbnail,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::reorder_collections,
            commands::list_tags,
            commands::add_tag,
            commands::remove_tag,
            commands::set_favorite,
            commands::set_description,
            commands::delete_meme,
            commands::batch_edit_memes,
            commands::delete_memes,
            commands::smart_copy,
            commands::collections_of_meme,
            commands::add_meme_to_collection,
            commands::remove_meme_from_collection,
            commands::get_config,
            commands::set_auto_paste,
            updater::get_updater_status,
            updater::set_update_mode,
            commands::complete_onboarding,
            commands::set_library_root,
            commands::set_theme,
            commands::probe_hotkey,
            commands::set_hotkey,
            commands::set_autostart,
            commands::get_storage_info,
            commands::clear_thumbnail_cache
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // 偏好配置：与应用配置目录（和图库数据库分离）
            let config_path = config::config_path(&app.path().app_config_dir()?);
            let cfg = config::load(&config_path);
            let library_root = cfg
                .library_root
                .clone()
                .map(PathBuf::from)
                .unwrap_or_else(default_library_root);
            let conn = open_db(&library_root)
                .map_err(|e| format!("打开表情库失败: {e}"))?;

            // 官方表情包：安装包内置贴纸，首启/换库/版本升级时自动导入
            // （失败只记日志，绝不阻塞应用启动）
            let stickers_dir = app
                .path()
                .resolve("resources/official", tauri::path::BaseDirectory::Resource)
                .unwrap_or_else(|_| PathBuf::from("resources/official"));
            match official::install_if_needed(&conn, &library_root, &stickers_dir) {
                Ok(Some(s)) => eprintln!(
                    "官方表情包导入完成: 新增 {}，跳过 {}，失败 {}",
                    s.imported, s.skipped, s.failed
                ),
                Ok(None) => {}
                Err(e) => eprintln!("官方表情包导入失败: {e}"),
            }
            let clipboard_temp =
                Mutex::new(smartcopy::TempStore::new(std::env::temp_dir().join("meme-manager-clipboard")));
            app.manage(AppState {
                conn: Mutex::new(conn),
                library_root: Mutex::new(library_root),
                thumb_queue: Mutex::new(HashSet::new()),
                clipboard_temp,
                summoner_focus: Mutex::new(None),
                config: Mutex::new(cfg.clone()),
                config_path,
            });

            // 托盘/菜单栏四项：打开、呼出、设置、退出
            let open = MenuItem::with_id(app, "open", "打开表情库", true, None::<&str>)?;
            let summon = MenuItem::with_id(app, "summon", "呼出 Quick Picker", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &summon, &settings, &quit])?;

            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("表情姬 MemeJi")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(main) = app.get_webview_window("main") {
                            let _ = main.show();
                            let _ = main.set_focus();
                        }
                    }
                    "summon" => toggle_quick_picker(app),
                    "settings" => {
                        if let Some(main) = app.get_webview_window("main") {
                            let _ = main.show();
                            let _ = main.set_focus();
                            let _ = app.emit_to("main", "open-settings", ());
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // 全局快捷键：读配置；失败在托盘明确提示，不静默
            let hotkey = cfg.hotkey.clone();
            if let Err(e) = apply_hotkey(&app.handle(), &hotkey) {
                eprintln!("全局快捷键注册失败: {e}");
                let _ = tray.set_tooltip(Some(format!("表情包管理器（快捷键不可用：{e}）")));
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // 退出全清：剪贴板临时副本目录
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Err(e) = state.clipboard_temp.lock().unwrap().wipe_all() {
                        eprintln!("清理剪贴板临时文件失败: {e}");
                    }
                }
            }
        });
}
