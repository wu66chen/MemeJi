# 应用更新发布配置

从 0.1.3 起，应用默认从 `wu66chen/MemeJi` 的正式 GitHub Release 读取 `latest.json`，并使用仓库中的公钥验证安装包。已发布的 0.1.2 没有更新源和公钥，因此必须先手动安装 0.1.3，之后的版本才能使用应用内更新。

1. 本机 Tauri updater 私钥保存在 `%USERPROFILE%\.memeji-signing\updater.key`，密码以 Windows 当前用户 DPAPI 加密保存在同目录的 `password.dpapi`，两者均在仓库外。发布者应在安全位置备份私钥和密码；丢失后旧安装版本将无法验证新密钥签署的更新。此签名独立于 Windows Authenticode 代码签名。
2. `src-tauri/tauri.conf.json` 已启用 `"createUpdaterArtifacts": true`，并配置下列 GitHub 源、公钥和签名版本检查：

   ```json
   "plugins": {
     "updater": {
       "pubkey": "<当前 updater.key.pub 的完整内容，不是路径>",
       "endpoints": [
         "https://github.com/wu66chen/MemeJi/releases/latest/download/latest.json"
       ],
       "windows": { "installMode": "passive" },
       "requireSignedVersion": true
     }
   }
   ```

3. 发布构建环境必须设置 `TAURI_SIGNING_PRIVATE_KEY`（私钥文件路径或内容）和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。本机的 `password.dpapi` 只允许当前 Windows 用户解密，可在构建进程内临时转成环境变量；构建完成后清除该变量。不要提交私钥、密码或明文 `.env`。`requireSignedVersion` 必须使用 **Tauri CLI 2.12.0 或更新版本**：旧 CLI 生成的签名缺少版本绑定，即使签名文件本身存在，应用也会拒绝安装。
4. 构建 Windows NSIS 安装器，生成 EXE、其 `.sig`、`SHA256SUMS.txt` 和 `latest.json` 后，先运行 `pnpm verify:release`；它会检查签名中的版本、清单 URL/签名和安装包哈希。四个文件须上传到同一个正式 Release。`latest.json` 至少包含 SemVer `version`、`platforms.windows-x86_64.url` 和该 EXE `.sig` 文件的**内容**作为 `signature`。上传后还须从公开 `/releases/latest/download/latest.json` 回读验证。未发布的草稿不会供 `/releases/latest/` 使用。
5. 在一台安装 0.1.3 的 Windows 机器上测试检查、确认、下载、签名验证、安装、重启和用户数据保留。Windows updater 安装时会退出当前应用；`passive` 模式显示安装进度。

如果发布 ARM64 或其他平台，在 `latest.json` 中加入对应的 `windows-aarch64` 等平台项和签名。自动更新签名不会改变 Windows SmartScreen 的「发布者未知」提示；那需要另行配置可信代码签名。

参考：[Tauri updater](https://v2.tauri.app/plugin/updater/)、[Tauri GitHub 发布流程](https://v2.tauri.app/distribute/pipelines/github/)、[Tauri 2.12 版本绑定签名说明](https://v2.tauri.app/release/tauri-cli/v2.12.0/)。
