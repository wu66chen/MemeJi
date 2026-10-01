# ADR-0001: 采用 Tauri 2（Rust 后端 + WebView 前端）

- 状态：已接受
- 日期：2026-08-30

## 背景

表情包管理器是本地优先、常驻后台、双平台（Windows + macOS）的轻量工具，核心能力依赖：跨平台剪贴板写入（含动图）、全局快捷键、系统托盘/菜单栏、多显示器光标锚点定位、SQLite、动图解码。候选：Tauri 2 / Electron / Flutter / Qt。

## 决策

采用 Tauri 2：Rust 后端 + 系统 WebView（WebView2 / WKWebView）+ Svelte 5 前端。

## 理由

- 轻量是硬约束：复用系统 WebView，包体与常驻内存四者最小；Electron 内嵌 Chromium 直接违背轻量原则
- 全局快捷键、托盘、窗口定位、NSIS/macOS 公证打包均为官方一等公民
- 动图剪贴板官方插件只支持静态图，但 Rust 后端可直连 Win32 `CF_HDROP` 与 macOS `public.file-url` 自研 Smart Copy，与 SQLite（rusqlite）、图片解码（image crate 生态）在同一语言闭环
- 前台应用识别/焦点恢复四框架均无官方 API，属非框架差异项

## 后果

- 需要自研两块原生模块：Smart Copy 动图写入、前台焦点恢复（一次性、低风险）
- 前端受各平台 WebView 引擎差异约束（macOS 动态 WebP 悬停播放需 Safari 14+/macOS 11+）
- Rust 工具链成为开发前置要求
