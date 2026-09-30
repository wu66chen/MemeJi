<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="MemeJi logo" width="128" />
</p>

<h1 align="center">MemeJi（表情姬）</h1>

<p align="center">Windows 本地表情包管理器 · 收藏整理 · Quick Picker · 自动粘贴</p>

MemeJi 是一款 Windows 本地表情包管理器。导入图片后，可以用收藏夹、分类组、标签和描述整理图库，再通过全局快捷键呼出 Quick Picker 搜索、复制并粘贴表情。

## 截图展示

<!-- 将截图保存到 docs/screenshots/，收到后以实际图片替换这些占位说明。 -->
<table>
  <tr>
    <td align="center" width="33%">
      <strong>图库与收藏夹分类</strong><br />
      <code>docs/screenshots/library.png</code><br />
      主窗口显示分类组、收藏夹和图库。
    </td>
    <td align="center" width="33%">
      <strong>Quick Picker</strong><br />
      <code>docs/screenshots/quick-picker.png</code><br />
      展示搜索、键盘导航和表情选择。
    </td>
    <td align="center" width="33%">
      <strong>设置</strong><br />
      <code>docs/screenshots/settings.png</code><br />
      展示自动粘贴与更新偏好选项。
    </td>
  </tr>
</table>

## 功能

- 托管本地图片库，按内容哈希去重
- 收藏夹支持分组；表情可加入多个收藏夹
- 按文件名、标签和描述搜索
- Quick Picker 支持键盘导航、鼠标附近呼出和快速搜索
- Smart Copy 为静态图写入位图格式，为动图保留文件引用
- 自动粘贴默认开启；仅在 Windows 能确认原输入控件仍可编辑时发送 Ctrl+V
- 内置 Mimu 表情包与系统托盘
- 更新设置支持关闭、检查后询问或自动安装；GitHub 发布源配置见 [更新发布配置](docs/updater-setup.md)

图库和偏好保存在本机，不会上传到服务端。

## 开发环境

- Windows 10/11
- Node.js 与 pnpm
- Rust stable 工具链
- Tauri 2 所需的 Windows C++ Build Tools 和 WebView2 Runtime

安装依赖并启动开发版：

```powershell
pnpm install
pnpm tauri dev
```

## 验证与打包

```powershell
pnpm check
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

Windows 安装包由 Tauri 生成在 `src-tauri/target/release/bundle/nsis/`。

## 文档


- [自动粘贴的焦点判定与限制](docs/quick-picker-auto-paste.md)
- [GitHub 更新发布配置](docs/updater-setup.md)


## 许可证

本项目基于 MIT License 发布，详见 [LICENSE](LICENSE)。
