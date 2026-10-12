mod commands;
pub mod config;
pub mod focus;
pub mod library;
pub mod official;
pub mod organize;
pub mod picker;
mod picker_session;
pub mod smartcopy;
pub mod thumbs;
pub mod updater;

use library::open_db;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent,
};

#[derive(Default)]
struct TrayClickState {
    generation: u64,
    suppress_until: Option<Instant>,
}
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub struct AppState {
    pub conn: Mutex<rusqlite::Connection>,
    /// 托管库根目录（向导里可改，改后立即换库）
    pub library_root: Mutex<PathBuf>,
    /// 进行中/排队中的缩略图生成（按 content hash 去重）
    pub thumb_queue: Mutex<HashSet<String>>,
    /// 剪贴板临时副本库（下次复制清上一批 + 退出全清）
    pub clipboard_temp: Mutex<smartcopy::TempStore>,
    /// 一次呼出、复制和取消共用同一个会话代数，旧任务不得操作新窗口。
    pub picker_session: Mutex<picker_session::PickerSession>,
    pub last_paste_status: Mutex<String>,
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
    gs.unregister_all()
        .map_err(|e| format!("解绑旧快捷键失败: {e}"))?;
    gs.register(accelerator)
        .map_err(|e| format!("快捷键 {accelerator} 注册失败（可能被其他程序占用）: {e}"))
}

fn toggle_quick_picker(app: &tauri::AppHandle) {
    let Some(picker) = app.get_webview_window("quick-picker") else {
        return;
    };
    let pending = app.state::<AppState>().picker_session.lock()
        .is_ok_and(|session| session.is_pending());
    if picker.is_visible().unwrap_or(false) || pending {
        dismiss_quick_picker(app, true);
    } else {
        let cursor = picker.cursor_position().ok();
        let original_focus = focus::capture();
        let generation = match app.state::<AppState>().picker_session.lock() {
            Ok(mut session) => session.begin(),
            Err(_) => return,
        };
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // UI Automation 查询可能阻塞目标应用；不能在窗口事件线程执行。
            let captured = tauri::async_runtime::spawn_blocking(move || {
                focus::enrich_for_paste(original_focus)
            }).await.ok().flatten();
            let state = app.state::<AppState>();
            if !state
                .picker_session
                .lock()
                .is_ok_and(|mut session| session.set_focus(generation, captured))
            {
                return;
            }
            let Some(picker) = app.get_webview_window("quick-picker") else {
                state.picker_session.lock().ok().map(|mut session| session.cancel());
                return;
            };
            if let Some(cursor) = cursor {
                position_picker(&picker, cursor);
            }
            let _ = picker.show();
            let _ = picker.set_focus();
            let _ = app.emit_to("quick-picker", "picker-shown", ());
        });
    }
}

fn dismiss_quick_picker(app: &tauri::AppHandle, restore_if_foreground: bool) {
    let focus = app
        .state::<AppState>()
        .picker_session
        .lock()
        .ok()
        .and_then(|mut session| session.cancel());
    if let Some(picker) = app.get_webview_window("quick-picker") {
        if restore_if_foreground && picker.is_focused().unwrap_or(false) {
            let _ = focus::restore(focus.as_ref());
        }
        let _ = picker.hide();
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn position_picker(picker: &WebviewWindow, cursor: PhysicalPosition<f64>) {
    let Ok(size) = picker.outer_size() else {
        return;
    };
    let Some(monitor) = picker.monitor_from_point(cursor.x, cursor.y).ok().flatten() else {
        return;
    };

    let work_area = monitor.work_area();
    let m_pos = work_area.position;
    let m_size = work_area.size;
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
    dismiss_quick_picker(&app, true);
}

#[tauri::command]
fn open_main_from_picker(app: tauri::AppHandle) {
    dismiss_quick_picker(&app, false);
    show_main(&app);
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
            show_main(app);
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
            open_main_from_picker,
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
            commands::set_wechat_compatibility_paste,
            commands::get_last_paste_status,
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
            if window.label() == "quick-picker" && matches!(event, WindowEvent::Focused(false)) {
                let app = window.app_handle();
                let should_hide = app.try_state::<AppState>().is_some_and(|state| {
                    state
                        .picker_session
                        .lock()
                        .is_ok_and(|mut session| session.cancel_on_blur())
                });
                if should_hide {
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
            let conn = open_db(&library_root).map_err(|e| format!("打开表情库失败: {e}"))?;

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
            let clipboard_temp = Mutex::new(smartcopy::TempStore::new(
                std::env::temp_dir().join("meme-manager-clipboard"),
            ));
            app.manage(AppState {
                conn: Mutex::new(conn),
                library_root: Mutex::new(library_root),
                thumb_queue: Mutex::new(HashSet::new()),
                clipboard_temp,
                picker_session: Mutex::new(picker_session::PickerSession::default()),
                last_paste_status: Mutex::new(String::new()),
                config: Mutex::new(cfg.clone()),
                config_path,
            });

            // 托盘/菜单栏四项：打开、呼出、设置、退出
            let open = MenuItem::with_id(app, "open", "打开表情库", true, None::<&str>)?;
            let summon = MenuItem::with_id(app, "summon", "呼出 Quick Picker", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &summon, &settings, &quit])?;
            let tray_clicks = Arc::new(Mutex::new(TrayClickState::default()));
            let event_clicks = tray_clicks.clone();

            let tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("表情姬 MemeJi")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        show_main(app);
                    }
                    "summon" => toggle_quick_picker(app),
                    "settings" => {
                        if app.get_webview_window("main").is_some() {
                            show_main(app);
                            let _ = app.emit_to("main", "open-settings", ());
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        let Ok(mut clicks) = event_clicks.lock() else {
                            return;
                        };
                        if clicks
                            .suppress_until
                            .is_some_and(|until| Instant::now() < until)
                        {
                            return;
                        }
                        clicks.generation = clicks.generation.wrapping_add(1);
                        let generation = clicks.generation;
                        drop(clicks);
                        let wait = unsafe {
                            windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime()
                        };
                        let tray = tray.clone();
                        let pending = event_clicks.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(wait as u64 + 30));
                            if pending
                                .lock()
                                .is_ok_and(|clicks| clicks.generation == generation)
                            {
                                let _ = tray.with_inner_tray_icon(|inner| inner.show_menu());
                            }
                        });
                    }
                    TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        if let Ok(mut clicks) = event_clicks.lock() {
                            clicks.generation = clicks.generation.wrapping_add(1);
                            let wait = unsafe {
                                windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime()
                            };
                            clicks.suppress_until =
                                Some(Instant::now() + Duration::from_millis(wait as u64));
                        }
                        show_main(&tray.app_handle());
                    }
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
