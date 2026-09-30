# Quick Picker 自动粘贴的判定边界

选择表情时，Smart Copy 先写入图片剪贴板，随后隐藏 Quick Picker、恢复呼出方窗口。设置中的「自动粘贴到原输入框」默认开启；关闭时仍会复制并恢复焦点。

## Windows 判定

1. 呼出前记录前台窗口句柄和进程 ID，同时用 UI Automation 读取当前键盘焦点控件。
2. 只接受已启用、有键盘焦点、非密码的 Edit 控件，或能证明非只读的 Document 控件。记录控件的 UI Automation runtime ID。
3. 选择表情后，恢复原窗口；再次确认前台窗口句柄和焦点控件 runtime ID 都与呼出前相同，并确认快捷键修饰键已松开。
4. 通过 `SendInput` 注入 Ctrl+V。只有完整注入才视为自动粘贴；任何校验失败都保留已写入的剪贴板，供用户手动粘贴。

聊天平台身份不参与判定。浏览器内聊天框与普通网页输入框都可以是 Edit 控件；通过进程名或窗口标题维护白名单既漏掉网页版，也不能证明当前控件可编辑。`GetForegroundWindow` 和 `GetGUIThreadInfo` 可用于窗口与原生焦点检查，但不能可靠辨认 WebView/Chromium 内部的输入元素。[UI Automation 的焦点元素与控件类型](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-obtainingelements)、[Edit 控件模式](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-supporteditcontroltype)提供了更具体的信息。

## 限制与人工验收

- 未提供可靠 UI Automation 身份的聊天输入框不会自动粘贴；仍可按 Ctrl+V。某些富文本 Document 控件不暴露可写模式，也会走此回退。
- `SendInput` 可能因目标程序权限高于本程序而被 Windows UIPI 拦截；系统不能从返回值准确区分这种情况。即使注入成功，也无法保证目标聊天软件接受图片或动画格式。[SendInput 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)。
- 目前只实现 Windows 自动粘贴；macOS 的 Smart Copy 与焦点恢复仍是项目原有的待办，不能据此认为 macOS 已支持该流程。
- 发布前用记事本、常用聊天客户端和浏览器聊天页面分别检查：编辑区聚焦时粘贴；非编辑区、密码框、窗口切换后只复制；静态图与 GIF 的实际粘贴结果。动态 WebP/APNG 的兼容性仍以目标软件为准。
