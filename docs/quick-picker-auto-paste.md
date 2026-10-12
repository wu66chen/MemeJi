# Quick Picker 自动粘贴的判定边界

选择表情时，Smart Copy 先写入图片剪贴板，随后隐藏 Quick Picker、恢复呼出方窗口。设置中的「自动粘贴到原输入框」默认开启；关闭时仍会复制并恢复焦点。

## Windows 判定

1. 呼出前记录前台窗口句柄和进程 ID，同时用 UI Automation 读取当前键盘焦点控件。
2. 只接受已启用、有键盘焦点、非密码的标准 Win32 编辑框，或能证明可写的 UI Automation 编辑控件。除 Edit / Document 外，也检查 Group / Custom / Pane / Text 包装控件的编辑接口：控件类型本身不代表可写。多行富文本框可以通过 `TextPattern` 的 `IsReadOnly` 布尔属性证明可写，无需同时支持 `ValuePattern` 或用于输入法组合的 `TextEditPattern`。任一接口明确报告只读时拒绝自动粘贴；不支持或混合的文本属性不会被当作“可写”。记录原生编辑框句柄或 UI Automation runtime ID。
3. 选择表情后，恢复原窗口；再次确认前台窗口句柄和焦点控件 runtime ID 都与呼出前相同，并确认快捷键修饰键已松开。
4. 通过 `SendInput` 注入 Ctrl+V。只有完整注入才视为自动粘贴；任何校验失败都保留已写入的剪贴板，供用户手动粘贴。

聊天平台身份不参与判定。浏览器内聊天框与普通网页输入框都可以是 Edit 控件；通过进程名或窗口标题维护白名单既漏掉网页版，也不能证明当前控件可编辑。`GetForegroundWindow` 和 `GetGUIThreadInfo` 可用于窗口与原生焦点检查，但不能可靠辨认 WebView/Chromium 内部的输入元素。[UI Automation 的焦点元素与控件类型](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-obtainingelements)、[Edit 控件模式](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporteditcontroltype)提供了更具体的信息。

若首次 UIA 焦点查询没有返回编辑控件，在原窗口内部查找报告键盘焦点的后代，并进行有限重试。同时通过 `AccessibleObjectFromWindow(OBJID_CLIENT)` 请求 MSAA 客户区、沿 `accFocus` 获取焦点对象，再转换为 UIA 元素。MSAA 只有在文本角色、已聚焦、可聚焦、非只读、非密码且可用时才能提供编辑证据；仍需 UIA runtime ID 和原窗口归属校验。整个过程不依赖鼠标停留位置，不读取文本，也不主动聚焦目标。[MSAA 对象查询](https://learn.microsoft.com/en-us/windows/win32/winauto/retrieving-an-iaccessible-object)、[MSAA 焦点对象](https://learn.microsoft.com/en-us/windows/win32/api/oleacc/nf-oleacc-iaccessible-get_accfocus)。

## 限制与人工验收

### 微信兼容模式（Windows，可选）

设置中的“微信兼容粘贴”默认关闭，并受“自动粘贴”总开关控制。它仅对 `Weixin.exe` / `WeChat.exe` 的 Qt 顶层窗口、且严格识别未取得编辑目标时生效；其他应用以及已识别的输入框沿用严格校验。

使用前先点中微信的目标输入框，再呼出 Quick Picker。兼容模式在复制后核对原窗口 HWND、PID、原生窗口焦点、微信进程和窗口类型、UIA 仍只报告原窗口、修饰键松开，再发送 Ctrl+V。它无法确认微信内部的聊天框、搜索框或其他区域，也无法发现同一窗口内的输入区切换。状态显示“已向原微信窗口发送兼容粘贴按键”，不表示已经验证输入框身份或图片被客户端接收。不会发送 Enter。

### 通用限制

- UIA 或 MSAA 路径无法提供可写证据和稳定控件身份时不会自动粘贴；仍可按 Ctrl+V。
- 输入框识别失败时，设置中的“最近一次粘贴”保留焦点属性、控件类型或可写接口缺失的原因，不记录输入文本、聊天内容、窗口标题或剪贴板内容。开发环境可运行 `cargo run --release --manifest-path src-tauri/Cargo.toml --example inspect-input`：在倒计时内点中目标输入框，诊断报告将写到该工具所在目录。
- `SendInput` 可能因目标程序权限高于本程序而被 Windows UIPI 拦截；系统不能从返回值准确区分这种情况。即使注入成功，也无法保证目标聊天软件接受图片或动画格式。[SendInput 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)。
- 目前只实现 Windows 自动粘贴；macOS 的 Smart Copy 与焦点恢复仍是项目原有的待办，不能据此认为 macOS 已支持该流程。
- 发布前用记事本、常用聊天客户端和浏览器聊天页面分别检查：编辑区聚焦时粘贴；非编辑区、密码框、窗口切换后只复制；静态图与 GIF 的实际粘贴结果。动态 WebP/APNG 的兼容性仍以目标软件为准。
