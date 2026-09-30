# 应用更新发布配置

应用已保留更新偏好和检查流程。当前项目没有 GitHub 远端，也没有发行版公钥，因此更新源状态为「未配置」，启动时不访问网络。发布前完成以下步骤。

1. 创建 GitHub 仓库，确定公开发行版地址和稳定版发布流程。
2. 用 `pnpm tauri signer generate -w <仓库外私钥路径>` 生成 Tauri updater 密钥。私钥及密码只放入安全保管处和 GitHub Actions Secrets，绝不提交。公钥可以提交。此签名独立于 Windows Authenticode 代码签名。
3. 在 `src-tauri/tauri.conf.json` 的 `bundle` 中加入 `"createUpdaterArtifacts": true`，并在根级加入：

   ```json
   "plugins": {
     "updater": {
       "pubkey": "<公钥文件的完整内容，不是路径>",
       "endpoints": [
         "https://github.com/<owner>/<repo>/releases/latest/download/latest.json"
       ],
       "windows": { "installMode": "passive" },
       "requireSignedVersion": true
     }
   }
   ```

4. 发布构建环境必须设置 `TAURI_SIGNING_PRIVATE_KEY`（私钥文件路径或内容）及有密码时的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。`.env` 文件不能代替构建环境变量。确认使用支持 `requireSignedVersion` 的 Tauri CLI/updater 版本，且签名带版本信息；不要降低此要求以迁就旧工具。
5. GitHub Actions 使用 `tauri-apps/tauri-action@v1`（或者等效发布脚本）构建 Windows NSIS 安装器，上传 EXE、其 `.sig` 和 `latest.json` 到同一个发行版。`latest.json` 至少包含 SemVer `version`、`platforms.windows-x86_64.url` 和该 EXE `.sig` 文件的**内容**作为 `signature`。先验证草稿，再发布稳定版；未发布的草稿不会供 `/releases/latest/` 使用。
6. 在一台安装旧版本的 Windows 机器上测试检查、确认、下载、签名验证、安装、重启和用户数据保留。Windows updater 安装时会退出当前应用；默认 `passive` 模式显示安装进度。不要使用 `quiet` 模式，它无法自行请求管理员权限。

如果发布 ARM64 或其他平台，在 `latest.json` 中加入对应的 `windows-aarch64` 等平台项和签名。丢失 updater 私钥后，已安装版本无法再验证用新密钥签署的更新，请做好密钥备份。

参考：[Tauri updater](https://v2.tauri.app/plugin/updater/)、[Tauri GitHub 发布流程](https://v2.tauri.app/distribute/pipelines/github/)、[Tauri 2.12 版本绑定签名说明](https://v2.tauri.app/release/tauri-cli/v2.12.0/)。
