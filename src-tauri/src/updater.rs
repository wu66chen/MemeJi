//! 更新偏好及发行源状态。实际检查、下载和安装使用官方 updater 插件。

use tauri::{AppHandle, State};

use crate::{config::UpdateMode, AppState};

#[derive(serde::Serialize)]
pub struct UpdaterStatus {
    pub configured: bool,
}

/// 只有配置了真实 HTTPS 更新源及公钥时才允许前端发起检查。
#[tauri::command]
pub fn get_updater_status(app: AppHandle) -> UpdaterStatus {
    let updater = app.config().plugins.0.get("updater");
    let pubkey = updater
        .and_then(|value| value.get("pubkey"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let has_https_endpoint = updater
        .and_then(|value| value.get("endpoints"))
        .and_then(|value| value.as_array())
        .is_some_and(|endpoints| {
            endpoints.iter().any(|endpoint| {
                endpoint
                    .as_str()
                    .is_some_and(|url| url.starts_with("https://"))
            })
        });
    UpdaterStatus {
        configured: !pubkey.trim().is_empty() && has_https_endpoint,
    }
}

#[tauri::command]
pub fn set_update_mode(state: State<'_, AppState>, mode: UpdateMode) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?.clone();
    config.update_mode = mode;
    state.save_config(&config)
}
