# Smart Copy 兼容矩阵 Checklist

> 平台原语：位图通道只承载单帧；文件引用（Windows CF_HDROP / macOS file-url）是可保留动画的通用通道。
> 实现策略：静态图 → CF_DIB + CF_DIBV5 + PNG 流；动图 → CF_HDROP 临时副本 + 首帧三格式兜底。
> **填写说明**：实测后在表格中填 ✅（保留动画 / 正常成图）、⚠️（降级为静态/首帧，符合兜底预期）、❌（失败）并注明日期与版本。

## 1. 动图矩阵（核心验收）

预期：GIF 普遍保留；动态 WebP / APNG 普遍不可靠（多数聊天软件不支持解码），⚠️ 属预期内。

### GIF

| 软件 | 平台 | 结果 | 备注 |
| --- | --- | --- | --- |
| 微信 | Windows | ⬜ 待测 | |
| 微信 | macOS | ⬜ 待测 | |
| QQ | Windows | ⬜ 待测 | |
| QQ | macOS | ⬜ 待测 | |
| Telegram | Windows | ⬜ 待测 | |
| Telegram | macOS | ⬜ 待测 | |
| Discord | Windows | ⬜ 待测 | |
| Discord | macOS | ⬜ 待测 | |
| Slack | Windows | ⬜ 待测 | |
| Slack | macOS | ⬜ 待测 | |
| Microsoft Teams | Windows | ⬜ 待测 | |
| Microsoft Teams | macOS | ⬜ 待测 | |

### 动态 WebP

| 软件 | 平台 | 结果 | 备注 |
| --- | --- | --- | --- |
| 微信 | Windows | ⬜ 待测 | |
| 微信 | macOS | ⬜ 待测 | |
| QQ | Windows | ⬜ 待测 | |
| QQ | macOS | ⬜ 待测 | |
| Telegram | Windows | ⬜ 待测 | |
| Telegram | macOS | ⬜ 待测 | |
| Discord | Windows | ⬜ 待测 | |
| Discord | macOS | ⬜ 待测 | |
| Slack | Windows | ⬜ 待测 | |
| Slack | macOS | ⬜ 待测 | |
| Microsoft Teams | Windows | ⬜ 待测 | |
| Microsoft Teams | macOS | ⬜ 待测 | |

### APNG

| 软件 | 平台 | 结果 | 备注 |
| --- | --- | --- | --- |
| 微信 | Windows | ⬜ 待测 | |
| 微信 | macOS | ⬜ 待测 | |
| QQ | Windows | ⬜ 待测 | |
| QQ | macOS | ⬜ 待测 | |
| Telegram | Windows | ⬜ 待测 | |
| Telegram | macOS | ⬜ 待测 | |
| Discord | Windows | ⬜ 待测 | |
| Discord | macOS | ⬜ 待测 | |
| Slack | Windows | ⬜ 待测 | |
| Slack | macOS | ⬜ 待测 | |
| Microsoft Teams | Windows | ⬜ 待测 | |
| Microsoft Teams | macOS | ⬜ 待测 | |

## 2. 静态图基线（PNG/JPG/静态 WebP/BMP）

预期：所有软件粘贴为位图 ✅（CF_DIB/DIBV5 + PNG 流）。

| 软件 | 平台 | 结果 | 备注 |
| --- | --- | --- | --- |
| 微信 | Windows | ⬜ 待测 | |
| QQ | Windows | ⬜ 待测 | |
| Telegram | Windows | ⬜ 待测 | |
| （其余按需扩展） | | | |

## 3. 兜底验证（不依赖聊天软件）

| 场景 | 预期 | 结果 |
| --- | --- | --- |
| 复制 GIF 后粘贴到「画图」/ Word | 得到首帧位图 | ⬜ 待测 |
| 复制 GIF 后粘贴到文件管理器/桌面 | 得到 .gif 临时副本文件 | ⬜ 待测 |
| 连续复制两次 | 临时目录只保留最新一份 | ⬜ 待测 |
| 应用退出后 | `%TEMP%/meme-manager-clipboard/` 清空 | ⬜ 待测 |
| 只读位图的应用（记事本类） | 剪贴板含 CF_DIB 可粘 | ⬜ 待测 |

## 4. 实测方法约定

- 呼出 → Enter 复制 → 回目标应用 → Ctrl/Cmd+V。
- 每格至少测 2 张样本（大 GIF >2MB、小 GIF <300KB）。
- macOS 通道（file-url + public.png）随打包票落地后补测。
