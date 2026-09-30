//! 应用偏好配置：与图库数据库分离的 JSON 配置文件（删库不丢偏好）。
//!
//! Seam：`load` / `save` / `config_path`。


use std::path::{Path, PathBuf};

pub const DEFAULT_HOTKEY_WIN: &str = "Ctrl+Shift+Space";
pub const DEFAULT_HOTKEY_MAC: &str = "Cmd+Shift+Space";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateMode {
    /// 完全关闭启动时自动检查；仍可由用户手动检查。
    Disabled,
    /// 自动检查，发现新版本后等待用户确认安装。
    Prompt,
    /// 自动检查并安装新版本（Windows 安装器会关闭应用）。
    Automatic,
}

impl Default for UpdateMode {
    fn default() -> Self {
        Self::Prompt
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)] // 旧文件缺字段 → 用默认值补齐
pub struct AppConfig {
    /// 是否完成过首启向导
    pub onboarded: bool,
    /// 全局快捷键（Tauri accelerator 字符串）
    pub hotkey: String,
    /// system | light | dark
    pub theme: String,
    /// 开机自启（OS 侧实际状态由 autostart 插件管理，此处作镜像）
    pub autostart: bool,
    /// 托管库路径；None = 用平台默认（~/Pictures/MemeLibrary）
    pub library_root: Option<String>,
    /// 更新方式：默认仅检查并提示。
    pub update_mode: UpdateMode,
    /// Smart Copy 后自动向原输入框发送粘贴快捷键。
    pub auto_paste: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            onboarded: false,
            hotkey: if cfg!(target_os = "macos") {
                DEFAULT_HOTKEY_MAC.into()
            } else {
                DEFAULT_HOTKEY_WIN.into()
            },
            theme: "system".into(),
            autostart: false,
            library_root: None,
            update_mode: UpdateMode::Prompt,
            auto_paste: true,
        }
    }
}

/// 配置文件路径：%APPDATA%/<identifier>/config.json（随应用配置目录，与库分离）。
pub fn config_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("config.json")
}

/// 读配置；文件缺失或损坏时返回默认值（绝不因此起不来应用）。
pub fn load(path: &Path) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 写配置（父目录不存在则创建）。
pub fn save(path: &Path, config: &AppConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips_all_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        let cfg = AppConfig {
            onboarded: true,
            hotkey: "Ctrl+Alt+M".into(),
            theme: "dark".into(),
            autostart: true,
            library_root: Some(r"D:\Memes".into()),
            update_mode: UpdateMode::Automatic,
            auto_paste: false,
        };
        save(&path, &cfg).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load(&tmp.path().join("不存在.json"));
        assert_eq!(cfg, AppConfig::default());
        assert!(!cfg.onboarded);
        assert_eq!(cfg.theme, "system");
        assert_eq!(cfg.update_mode, UpdateMode::Prompt);
        assert!(cfg.auto_paste);
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{ "onboarded": true }"#).unwrap();
        let cfg = load(&path);
        assert!(cfg.onboarded);
        assert_eq!(cfg.hotkey, AppConfig::default().hotkey);
        assert_eq!(cfg.theme, "system");
        assert_eq!(cfg.update_mode, UpdateMode::Prompt);
        assert!(cfg.auto_paste);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, "not json at all {").unwrap();
        assert_eq!(load(&path), AppConfig::default());
    }

    #[test]
    fn disabled_update_mode_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        let mut cfg = AppConfig::default();
        cfg.update_mode = UpdateMode::Disabled;
        save(&path, &cfg).unwrap();
        assert_eq!(load(&path).update_mode, UpdateMode::Disabled);
    }
}
