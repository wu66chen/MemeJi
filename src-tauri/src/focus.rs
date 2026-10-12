//! 焦点捕获与恢复：呼出前记录前台应用，复制后把焦点还给呼出方。
//!
//! Seam：`capture` / `restore`。
//!
//! Windows：呼出瞬间记录前台 HWND + pid；恢复仅尝试同一窗口。
//! macOS：记录 frontmostApplication（bundle id + pid）恢复 activate —— 随打包票落地。
//! 自动粘贴为 P1 功能：Windows 使用只读 UI Automation 查询焦点控件身份；
//! 焦点恢复本身仍不依赖辅助功能 API。识别失败时保留复制结果供手工粘贴。

#[derive(Debug, Clone)]
pub struct CapturedFocus {
    /// Windows：HWND；其他平台：0
    hwnd: isize,
    /// 前台窗口所属进程，0 表示未知
    pid: u32,
    /// UI Automation 的可编辑控件身份；默认不可确认时只复制。
    paste_target: Option<PasteTarget>,
    /// Accessibility metadata only; never input content or window titles.
    input_diagnostic: String,
    /// Window focus only; this is never treated as editable input proof.
    wechat_compatibility_target: Option<isize>,
}

#[derive(Debug, Clone, Copy)]
pub enum PasteFailure {
    NoOriginalWindow,
    NoEditableTarget,
    WindowChanged,
    InputChanged,
    ModifierHeld,
    SystemRejected,
    Cancelled,
}

impl PasteFailure {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::NoOriginalWindow => "未记录原窗口",
            Self::NoEditableTarget => "未识别到可编辑输入框",
            Self::WindowChanged => "焦点未回到原窗口",
            Self::InputChanged => "原输入框焦点已变化",
            Self::ModifierHeld => "快捷键修饰键尚未松开",
            Self::SystemRejected => "系统未接受粘贴按键",
            Self::Cancelled => "本次操作已取消",
        }
    }
}

pub fn has_editable_target(focus: Option<&CapturedFocus>) -> bool {
    focus.is_some_and(|value| value.paste_target.is_some())
}

pub fn input_diagnostic(focus: Option<&CapturedFocus>) -> &str {
    focus
        .map(|value| value.input_diagnostic.as_str())
        .unwrap_or("未记录原窗口")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PasteTarget {
    hwnd: isize,
    identity: EditIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EditIdentity {
    Uia(Vec<i32>),
    Native(isize),
}

fn may_paste(target: &PasteTarget, foreground_hwnd: isize, focused: Option<&EditIdentity>) -> bool {
    target.hwnd == foreground_hwnd && focused == Some(&target.identity)
}

fn is_writable_uia_edit(
    value_read_only: Option<bool>,
    text_read_only: Option<bool>,
    supports_text_edit: bool,
) -> bool {
    if value_read_only == Some(true) || text_read_only == Some(true) {
        return false;
    }
    value_read_only == Some(false) || text_read_only == Some(false) || supports_text_edit
}

fn is_edit_container_kind(kind: i32) -> bool {
    // Edit, Document, Group, Custom, Pane, Text. Kind alone never proves writability.
    [50004, 50030, 50026, 50025, 50033, 50020].contains(&kind)
}

fn is_writable_legacy_text(role: u32, state: u32) -> bool {
    // ROLE_SYSTEM_TEXT; focused + focusable; deny unavailable, readonly,
    // invisible, offscreen and protected. Never inspect accValue/accName.
    role == 42
        && state & 0x100004 == 0x100004
        && state & (0x1 | 0x40 | 0x8000 | 0x10000 | 0x20000000) == 0
}

#[derive(Debug, Clone, Copy)]
pub enum PasteMode {
    VerifiedInput,
    WechatCompatibility,
}

pub fn has_wechat_compatibility_target(focus: Option<&CapturedFocus>) -> bool {
    focus.is_some_and(|value| value.wechat_compatibility_target.is_some())
}

fn is_wechat_compatibility_host(executable: &str, class: &str) -> bool {
    (executable.eq_ignore_ascii_case("Weixin.exe") || executable.eq_ignore_ascii_case("WeChat.exe"))
        && class
            .strip_prefix("Qt")
            .and_then(|value| value.strip_suffix("QWindowIcon"))
            .is_some_and(|version| {
                !version.is_empty() && version.bytes().all(|value| value.is_ascii_digit())
            })
}

fn may_use_wechat_compatibility(
    enabled: bool,
    has_editable_target: bool,
    original_native: Option<isize>,
    current_native: Option<isize>,
) -> bool {
    enabled
        && !has_editable_target
        && original_native.is_some_and(|value| value != 0)
        && original_native == current_native
}

fn run_if_current<T>(
    is_current: impl Fn() -> bool,
    action: impl FnOnce() -> Result<T, PasteFailure>,
) -> Result<T, PasteFailure> {
    if !is_current() {
        return Err(PasteFailure::Cancelled);
    }
    action()
}

fn may_restore(
    original_hwnd: isize,
    original_pid: u32,
    foreground_hwnd: isize,
    foreground_pid: u32,
    our_pid: u32,
) -> bool {
    original_hwnd != 0
        && original_pid != 0
        && original_pid != our_pid
        && (foreground_hwnd == original_hwnd || foreground_pid == our_pid)
}

/// 快速记录当前前台窗口（应在呼出 Quick Picker **之前**调用）。
#[cfg(windows)]
pub fn capture() -> Option<CapturedFocus> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        Some(CapturedFocus {
            hwnd: hwnd.0 as isize,
            pid,
            paste_target: None,
            input_diagnostic: "尚未完成输入框识别".to_string(),
            wechat_compatibility_target: None,
        })
    }
}

/// UI Automation 查询在 worker 中执行；若呼出后前台已变化，只保留原窗口快照。
#[cfg(windows)]
pub fn enrich_for_paste(mut focus: Option<CapturedFocus>) -> Option<CapturedFocus> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let Some(ref mut captured) = focus else {
        return None;
    };
    let hwnd = HWND(captured.hwnd as *mut _);
    let foreground_matches = || unsafe {
        let foreground = GetForegroundWindow();
        let mut pid = 0u32;
        GetWindowThreadProcessId(foreground, Some(&mut pid));
        foreground == hwnd && pid == captured.pid
    };
    if foreground_matches() {
        let mut diagnostic = String::new();
        let identity =
            windows_impl::focused_edit_target_with_diagnostic(hwnd, captured.pid, &mut diagnostic);
        if foreground_matches() {
            captured.paste_target = identity.map(|identity| PasteTarget {
                hwnd: captured.hwnd,
                identity,
            });
            captured.input_diagnostic = diagnostic;
            if captured.paste_target.is_none() {
                captured.wechat_compatibility_target =
                    windows_impl::opaque_wechat_focus(hwnd, captured.pid);
            }
        } else {
            captured.input_diagnostic = "识别时前台窗口已变化".to_string();
        }
    } else {
        captured.input_diagnostic = "识别时前台窗口已变化".to_string();
    }
    focus
}

#[cfg(not(windows))]
pub fn capture() -> Option<CapturedFocus> {
    None
}

#[cfg(not(windows))]
pub fn enrich_for_paste(focus: Option<CapturedFocus>) -> Option<CapturedFocus> {
    focus
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        is_edit_container_kind, is_wechat_compatibility_host, is_writable_legacy_text,
        is_writable_uia_edit, may_paste, may_restore, may_use_wechat_compatibility, CapturedFocus,
        EditIdentity, PasteFailure,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    };

    pub fn restore(focus: Option<&CapturedFocus>) -> Result<(), PasteFailure> {
        restore_if_current(focus, || true)
    }

    pub fn restore_if_current(
        focus: Option<&CapturedFocus>,
        is_current: impl Fn() -> bool,
    ) -> Result<(), PasteFailure> {
        let Some(focus) = focus else {
            return Err(PasteFailure::NoOriginalWindow);
        };
        unsafe {
            let hwnd = HWND(focus.hwnd as *mut _);
            let mut current_pid = 0u32;
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(PasteFailure::NoOriginalWindow);
            }
            GetWindowThreadProcessId(hwnd, Some(&mut current_pid));
            if current_pid != focus.pid {
                return Err(PasteFailure::NoOriginalWindow);
            }

            let fg = GetForegroundWindow();
            if fg.0.is_null() {
                return Err(PasteFailure::WindowChanged);
            }
            let mut fg_pid = 0u32;
            let fg_thread = GetWindowThreadProcessId(fg, Some(&mut fg_pid));
            if !may_restore(
                focus.hwnd,
                focus.pid,
                fg.0 as isize,
                fg_pid,
                std::process::id(),
            ) {
                return Err(PasteFailure::WindowChanged);
            }
            if fg == hwnd {
                return Ok(());
            }

            // 仅本应用仍在前台时尝试激活原 HWND，不跟随进程号寻找其他窗口。
            let this_thread = GetCurrentThreadId();
            let target_thread = GetWindowThreadProcessId(hwnd, None);

            let attached_fg = fg_thread != 0 && fg_thread != this_thread;
            if attached_fg {
                let _ = AttachThreadInput(this_thread, fg_thread, true);
            }
            let attached_target = target_thread != 0 && target_thread != this_thread;
            if attached_target {
                let _ = AttachThreadInput(this_thread, target_thread, true);
            }
            let restored = super::run_if_current(is_current, || {
                let _ = SetForegroundWindow(hwnd);
                Ok(())
            });
            if attached_fg {
                let _ = AttachThreadInput(this_thread, fg_thread, false);
            }
            if attached_target {
                let _ = AttachThreadInput(this_thread, target_thread, false);
            }
            restored?;
            if GetForegroundWindow() == hwnd {
                Ok(())
            } else {
                Err(PasteFailure::WindowChanged)
            }
        }
    }

    /// 只接受同一前台窗口中可证明可编辑的 UIA 控件或标准 Win32 编辑框。
    pub fn focused_edit_target(hwnd: HWND, pid: u32) -> Option<EditIdentity> {
        focused_edit_target_with_diagnostic(hwnd, pid, &mut String::new())
    }

    pub fn focused_edit_target_with_diagnostic(
        hwnd: HWND,
        pid: u32,
        diagnostic: &mut String,
    ) -> Option<EditIdentity> {
        if unsafe { GetForegroundWindow() } != hwnd {
            *diagnostic = "识别时前台窗口已变化".to_string();
            return None;
        }
        let identity = if let Some(native) = focused_native_edit(hwnd, pid) {
            *diagnostic = "标准 Win32 可写编辑框".to_string();
            Some(EditIdentity::Native(native))
        } else {
            focused_uia_edit_id(hwnd, diagnostic).map(EditIdentity::Uia)
        };
        identity.filter(|_| unsafe { GetForegroundWindow() } == hwnd)
    }

    fn focused_native_edit(hwnd: HWND, pid: u32) -> Option<isize> {
        use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetClassNameW, GetGUIThreadInfo, GetWindowLongW, ES_PASSWORD, ES_READONLY,
            GUITHREADINFO, GWL_STYLE,
        };
        unsafe {
            if GetForegroundWindow() != hwnd {
                return None;
            }
            let thread = GetWindowThreadProcessId(hwnd, None);
            if thread == 0 {
                return None;
            }
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            GetGUIThreadInfo(thread, &mut info).ok()?;
            let focused = info.hwndFocus;
            if focused.0.is_null() || !IsWindowEnabled(focused).as_bool() {
                return None;
            }
            let mut focus_pid = 0u32;
            GetWindowThreadProcessId(focused, Some(&mut focus_pid));
            if focus_pid != pid {
                return None;
            }
            let mut class = [0u16; 128];
            let len = GetClassNameW(focused, &mut class);
            if len <= 0 {
                return None;
            }
            let class = String::from_utf16_lossy(&class[..len as usize]).to_ascii_lowercase();
            if class != "edit" && !class.starts_with("richedit") {
                return None;
            }
            let style = GetWindowLongW(focused, GWL_STYLE);
            if style & (ES_PASSWORD | ES_READONLY) != 0 {
                return None;
            }
            Some(focused.0 as isize)
        }
    }

    // Root all fallback discovery in the original HWND. No cursor dependency,
    // application whitelist, focus setting, input text or arbitrary key injection.
    fn focused_uia_edit_id(hwnd: HWND, diagnostic: &mut String) -> Option<Vec<i32>> {
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Accessibility::{
            CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_HasKeyboardFocusPropertyId,
        };
        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let result = (|| {
                let automation: IUIAutomation =
                    match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                        Ok(value) => value,
                        Err(error) => {
                            *diagnostic = format!("UIA 初始化失败（{}）", error.code());
                            return None;
                        }
                    };
                let root = automation.ElementFromHandle(hwnd).ok()?;
                let mut last = String::new();
                for attempt in 0..3 {
                    if GetForegroundWindow() != hwnd {
                        *diagnostic = "识别时前台窗口已变化".into();
                        return None;
                    }
                    if let Ok(focused) = automation.GetFocusedElement() {
                        if let Some(ids) =
                            editable_uia_identity(&automation, &root, &focused, &mut last)
                        {
                            *diagnostic = last;
                            return Some(ids);
                        }
                    }
                    // Chrome can initially report a non-focused outer View while
                    // the focused HTML editor already exists below the window root.
                    if let Ok(condition) = automation
                        .CreatePropertyCondition(UIA_HasKeyboardFocusPropertyId, &true.into())
                    {
                        if let Ok(focused) = root.FindFirst(TreeScope_Descendants, &condition) {
                            if let Some(ids) =
                                editable_uia_identity(&automation, &root, &focused, &mut last)
                            {
                                *diagnostic = last;
                                return Some(ids);
                            }
                        }
                    }
                    // Qt 5 may expose its editor through MSAA rather than UIA's
                    // default window proxy. This query also requests lazy providers.
                    let mut msaa_diagnostic = String::new();
                    if let Some(focused) =
                        focused_msaa_element(&automation, hwnd, &mut msaa_diagnostic)
                    {
                        if let Some(ids) =
                            editable_uia_identity(&automation, &root, &focused, &mut last)
                        {
                            *diagnostic = format!("MSAA→{last}");
                            return Some(ids);
                        }
                    }
                    *diagnostic = format!("{last}；{msaa_diagnostic}");
                    if attempt < 2 {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                    }
                }
                None
            })();
            if initialized {
                CoUninitialize();
            }
            result
        }
    }

    // Read-only MSAA focused-object navigation. Convert to UIA so both capture
    // and final recheck use the same stable runtime identity, not COM addresses.
    fn focused_msaa_element(
        automation: &windows::Win32::UI::Accessibility::IUIAutomation,
        hwnd: HWND,
        diagnostic: &mut String,
    ) -> Option<windows::Win32::UI::Accessibility::IUIAutomationElement> {
        use windows::core::Interface;
        use windows::Win32::System::Variant::{VARIANT, VT_DISPATCH, VT_I4};
        use windows::Win32::UI::Accessibility::{AccessibleObjectFromWindow, IAccessible};
        use windows::Win32::UI::WindowsAndMessaging::OBJID_CLIENT;
        unsafe {
            let mut raw = std::ptr::null_mut();
            if let Err(error) =
                AccessibleObjectFromWindow(hwnd, OBJID_CLIENT.0 as u32, &IAccessible::IID, &mut raw)
            {
                *diagnostic = format!("MSAA 客户区不可用（{}）", error.code());
                return None;
            }
            if raw.is_null() {
                *diagnostic = "MSAA 未返回客户区".into();
                return None;
            }
            let mut object = IAccessible::from_raw(raw);
            for _ in 0..8 {
                // Leaf providers may return VT_EMPTY or their own dispatch from
                // accFocus. Recognize a proven focused editor before descending.
                let self_id = VARIANT::from(0i32);
                let role = object
                    .get_accRole(&self_id)
                    .ok()
                    .and_then(|value| u32::try_from(&value).ok());
                let state = object
                    .get_accState(&self_id)
                    .ok()
                    .and_then(|value| u32::try_from(&value).ok());
                *diagnostic = format!("MSAA role={role:?}，state={state:?}");
                if role
                    .zip(state)
                    .is_some_and(|(role, state)| is_writable_legacy_text(role, state))
                {
                    return automation.ElementFromIAccessible(&object, 0).ok();
                }
                let focus = match object.accFocus() {
                    Ok(value) => value,
                    Err(error) => {
                        *diagnostic = format!("MSAA 无法取得焦点（{}）", error.code());
                        return None;
                    }
                };
                let variant = &focus.Anonymous.Anonymous;
                if variant.vt == VT_DISPATCH {
                    object = variant.Anonymous.pdispVal.as_ref()?.cast().ok()?;
                    continue;
                }
                if variant.vt == VT_I4 {
                    let child = i32::try_from(&focus).ok()?;
                    if child < 0 {
                        return None;
                    }
                    let child_variant = VARIANT::from(child);
                    let role = object
                        .get_accRole(&child_variant)
                        .ok()
                        .and_then(|value| u32::try_from(&value).ok());
                    let state = object
                        .get_accState(&child_variant)
                        .ok()
                        .and_then(|value| u32::try_from(&value).ok());
                    *diagnostic = format!("MSAA 焦点 role={role:?}，state={state:?}");
                    return automation.ElementFromIAccessible(&object, child).ok();
                }
                *diagnostic = format!("MSAA 无焦点对象（类型={}）", variant.vt.0);
                return None;
            }
            *diagnostic = "MSAA 焦点链超过上限".into();
            None
        }
    }

    fn editable_uia_identity(
        automation: &windows::Win32::UI::Accessibility::IUIAutomation,
        root: &windows::Win32::UI::Accessibility::IUIAutomationElement,
        focused: &windows::Win32::UI::Accessibility::IUIAutomationElement,
        diagnostic: &mut String,
    ) -> Option<Vec<i32>> {
        use windows::Win32::System::Ole::{
            SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
            SafeArrayGetUBound,
        };
        use windows::Win32::System::Variant::VT_BOOL;
        use windows::Win32::UI::Accessibility::*;
        unsafe {
            *diagnostic = "UIA 控件不属于原窗口".into();
            let walker = automation.RawViewWalker().ok()?;
            let mut ancestor = focused.clone();
            let mut belongs = false;
            for _ in 0..64 {
                if automation.CompareElements(&ancestor, root).ok()?.as_bool() {
                    belongs = true;
                    break;
                }
                ancestor = match walker.GetParentElement(&ancestor) {
                    Ok(value) => value,
                    Err(_) => break,
                };
            }
            if !belongs {
                return None;
            }
            let kind = focused.CurrentControlType().ok()?.0;
            let has_focus = focused.CurrentHasKeyboardFocus().ok()?.as_bool();
            let focusable = focused.CurrentIsKeyboardFocusable().ok()?.as_bool();
            let enabled = focused.CurrentIsEnabled().ok()?.as_bool();
            let password = focused.CurrentIsPassword().ok()?.as_bool();
            *diagnostic = format!("UIA type={kind}，焦点={has_focus}，可聚焦={focusable}，启用={enabled}，密码={password}");
            if !enabled || password || !has_focus || !focusable || !is_edit_container_kind(kind) {
                return None;
            }
            let mut element = focused.clone();
            let mut proven = false;
            for depth in 0..=4 {
                let kind = element.CurrentControlType().ok()?.0;
                if !is_edit_container_kind(kind) {
                    break;
                }
                let value_read_only = element
                    .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                    .ok()
                    .and_then(|pattern| pattern.CurrentIsReadOnly().ok())
                    .map(|value| value.as_bool());
                let text_read_only = element
                    .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                    .ok()
                    .and_then(|pattern| pattern.DocumentRange().ok())
                    .and_then(|range| range.GetAttributeValue(UIA_IsReadOnlyAttributeId).ok())
                    .filter(|value| value.Anonymous.Anonymous.vt == VT_BOOL)
                    .and_then(|value| bool::try_from(&value).ok());
                let text_edit = element
                    .GetCurrentPatternAs::<IUIAutomationTextEditPattern>(UIA_TextEditPatternId)
                    .is_ok();
                let legacy = element
                    .GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
                        UIA_LegacyIAccessiblePatternId,
                    )
                    .ok()
                    .and_then(|pattern| {
                        Some((pattern.CurrentRole().ok()?, pattern.CurrentState().ok()?))
                    });
                *diagnostic = format!("UIA type={kind}：Value只读={value_read_only:?}，Text只读={text_read_only:?}，TextEdit={text_edit}，MSAA={legacy:?}");
                // Explicit readonly/password/disabled evidence always wins, including
                // Legacy providers that also expose a writable-looking text pattern.
                if value_read_only == Some(true)
                    || text_read_only == Some(true)
                    || legacy.is_some_and(|(_, state)| state & (0x1 | 0x40 | 0x20000000) != 0)
                {
                    return None;
                }
                if is_writable_uia_edit(value_read_only, text_read_only, text_edit)
                    || legacy.is_some_and(|(role, state)| is_writable_legacy_text(role, state))
                {
                    proven = true;
                    break;
                }
                // Retain focused text/custom wrapper support: prove its enclosing
                // editor, but always record the original focused element's identity.
                if depth == 4 || ![50020, 50025, 50033].contains(&kind) {
                    break;
                }
                element = walker.GetParentElement(&element).ok()?;
                if !element.CurrentIsEnabled().ok()?.as_bool()
                    || element.CurrentIsPassword().ok()?.as_bool()
                {
                    return None;
                }
            }
            if !proven {
                return None;
            }
            *diagnostic = "UIA 可写编辑框缺少稳定控件身份".into();
            let array = focused.GetRuntimeId().ok()?;
            if array.is_null() {
                return None;
            }
            let ids = (|| {
                if SafeArrayGetDim(array) != 1 {
                    return None;
                }
                let lower = SafeArrayGetLBound(array, 1).ok()?;
                let upper = SafeArrayGetUBound(array, 1).ok()?;
                if upper < lower || upper.checked_sub(lower)? > 128 {
                    return None;
                }
                let mut values = Vec::new();
                for index in lower..=upper {
                    let mut value = 0i32;
                    SafeArrayGetElement(array, &index, &mut value as *mut _ as *mut _).ok()?;
                    values.push(value);
                }
                Some(values)
            })();
            let _ = SafeArrayDestroy(array);
            if ids.is_some() {
                *diagnostic = format!("UIA type={kind} 可写输入框和身份已确认");
            }
            ids
        }
    }

    /// Only expose the compatibility path for an opaque WeChat host. Native
    /// focus must remain its top-level HWND and UIA must still report that window.
    pub fn opaque_wechat_focus(hwnd: HWND, pid: u32) -> Option<isize> {
        use windows::core::PWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows::Win32::UI::Accessibility::{
            CUIAutomation, IUIAutomation, UIA_WindowControlTypeId,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetClassNameW, GetGUIThreadInfo, GUITHREADINFO,
        };
        unsafe {
            if GetForegroundWindow() != hwnd || !IsWindowEnabled(hwnd).as_bool() {
                return None;
            }
            let mut current_pid = 0;
            let thread = GetWindowThreadProcessId(hwnd, Some(&mut current_pid));
            if pid == 0 || current_pid != pid || thread == 0 {
                return None;
            }
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            GetGUIThreadInfo(thread, &mut info).ok()?;
            if info.hwndActive != hwnd || info.hwndFocus != hwnd {
                return None;
            }
            let mut class = [0u16; 128];
            let len = GetClassNameW(hwnd, &mut class);
            if len <= 0 {
                return None;
            }
            let class = String::from_utf16_lossy(&class[..len as usize]);
            if !class.starts_with("Qt") {
                return None;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut path = vec![0u16; 32768];
            let mut len = path.len() as u32;
            let query = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                PWSTR(path.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(process);
            query.ok()?;
            let path = String::from_utf16_lossy(&path[..len as usize]);
            let executable = std::path::Path::new(&path).file_name()?.to_str()?;
            if !is_wechat_compatibility_host(executable, &class) {
                return None;
            }
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let result = (|| {
                let automation: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                let element = automation.GetFocusedElement().ok()?;
                if element.CurrentControlType().ok()? != UIA_WindowControlTypeId
                    || element.CurrentNativeWindowHandle().ok()? != hwnd
                    || !element.CurrentHasKeyboardFocus().ok()?.as_bool()
                    || !element.CurrentIsEnabled().ok()?.as_bool()
                    || element.CurrentIsPassword().ok()?.as_bool()
                    || GetForegroundWindow() != hwnd
                {
                    return None;
                }
                Some(hwnd.0 as isize)
            })();
            if initialized {
                CoUninitialize();
            }
            result
        }
    }

    pub fn paste_if_editable(focus: Option<&CapturedFocus>) -> Result<(), super::PasteFailure> {
        paste_with_policy(focus, false, || true).map(|_| ())
    }

    pub fn paste_with_policy(
        focus: Option<&CapturedFocus>,
        allow_wechat: bool,
        is_current: impl Fn() -> bool,
    ) -> Result<super::PasteMode, super::PasteFailure> {
        use super::PasteFailure;
        use super::PasteMode;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
        };
        let Some(focus) = focus else {
            return Err(PasteFailure::NoOriginalWindow);
        };
        if focus.pid == std::process::id() {
            return Err(PasteFailure::NoOriginalWindow);
        }
        let foreground = unsafe { GetForegroundWindow() };
        let foreground_hwnd = foreground.0 as isize;
        if foreground_hwnd != focus.hwnd {
            return Err(PasteFailure::WindowChanged);
        }
        let mut foreground_pid = 0u32;
        unsafe { GetWindowThreadProcessId(foreground, Some(&mut foreground_pid)) };
        if foreground_pid != focus.pid {
            return Err(PasteFailure::WindowChanged);
        }
        let mode = if let Some(target) = &focus.paste_target {
            if !may_paste(
                target,
                foreground_hwnd,
                focused_edit_target(foreground, focus.pid).as_ref(),
            ) {
                return Err(PasteFailure::InputChanged);
            }
            PasteMode::VerifiedInput
        } else {
            if !allow_wechat || focus.wechat_compatibility_target.is_none() {
                return Err(PasteFailure::NoEditableTarget);
            }
            if !may_use_wechat_compatibility(
                allow_wechat,
                false,
                focus.wechat_compatibility_target,
                opaque_wechat_focus(foreground, focus.pid),
            ) {
                return Err(PasteFailure::InputChanged);
            }
            PasteMode::WechatCompatibility
        };
        // 用户还按着快捷键修饰键时不注入，避免组合成其他操作。
        for key in [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN] {
            if unsafe { GetAsyncKeyState(key.0 as i32) } < 0 {
                return Err(PasteFailure::ModifierHeld);
            }
        }
        let key = |vk, up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        Default::default()
                    },
                    ..Default::default()
                },
            },
        };
        // 最后再确认前台没变；SendInput 仍可能被 UIPI 拦截，失败时剪贴板可手工粘贴。
        let final_foreground = unsafe { GetForegroundWindow() };
        let mut final_pid = 0u32;
        unsafe { GetWindowThreadProcessId(final_foreground, Some(&mut final_pid)) };
        if final_foreground.0 as isize != focus.hwnd || final_pid != focus.pid {
            return Err(PasteFailure::WindowChanged);
        }
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        super::run_if_current(is_current, || {
            let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
            if sent != inputs.len() as u32 && sent > 0 {
                // 若仅注入了前缀，补发释放事件，避免 Ctrl 卡在按下状态。
                let release = [key(VK_V, true), key(VK_CONTROL, true)];
                unsafe {
                    SendInput(&release, std::mem::size_of::<INPUT>() as i32);
                }
            }
            if sent == inputs.len() as u32 {
                Ok(mode)
            } else {
                Err(PasteFailure::SystemRejected)
            }
        })
    }
}

#[cfg(windows)]
pub use windows_impl::{paste_if_editable, paste_with_policy, restore, restore_if_current};

#[cfg(not(windows))]
pub fn restore(_focus: Option<&CapturedFocus>) -> Result<(), PasteFailure> {
    Err(PasteFailure::NoOriginalWindow)
}

#[cfg(not(windows))]
pub fn restore_if_current(
    focus: Option<&CapturedFocus>,
    is_current: impl Fn() -> bool,
) -> Result<(), PasteFailure> {
    run_if_current(is_current, || restore(focus))
}

#[cfg(not(windows))]
pub fn paste_if_editable(_focus: Option<&CapturedFocus>) -> Result<(), PasteFailure> {
    Err(PasteFailure::NoEditableTarget)
}

#[cfg(not(windows))]
pub fn paste_with_policy(
    _focus: Option<&CapturedFocus>,
    _allow_wechat: bool,
    _is_current: impl Fn() -> bool,
) -> Result<PasteMode, PasteFailure> {
    Err(PasteFailure::NoEditableTarget)
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn invalidation_during_input_query_prevents_final_native_action() {
        use std::cell::Cell;
        let generation = Cell::new(1);
        let sent = Cell::new(false);
        let captured_generation = generation.get();
        // The blocking accessibility lookup finished after a new summon began.
        generation.set(2);
        let outcome = run_if_current(
            || generation.get() == captured_generation,
            || {
                sent.set(true);
                Ok(())
            },
        );
        assert!(outcome.is_err());
        assert!(!sent.get());
        assert!(run_if_current(|| true, || Ok(())).is_ok());
    }

    #[test]
    fn wechat_compatibility_excludes_other_qt_apps_and_wechat_webviews() {
        assert!(is_wechat_compatibility_host(
            "Weixin.exe",
            "Qt51514QWindowIcon"
        ));
        assert!(is_wechat_compatibility_host(
            "WECHAT.EXE",
            "Qt5152QWindowIcon"
        ));
        assert!(!is_wechat_compatibility_host(
            "other.exe",
            "Qt51514QWindowIcon"
        ));
        assert!(!is_wechat_compatibility_host(
            "WeixinAppEx.exe",
            "Qt51514QWindowIcon"
        ));
        assert!(!is_wechat_compatibility_host(
            "Weixin.exe",
            "Chrome_WidgetWin_1"
        ));
        assert!(!is_wechat_compatibility_host("Weixin.exe", "QtQWindowIcon"));
    }

    #[test]
    fn wechat_compatibility_is_opt_in_and_requires_unchanged_native_focus() {
        assert!(may_use_wechat_compatibility(
            true,
            false,
            Some(42),
            Some(42)
        ));
        assert!(!may_use_wechat_compatibility(
            false,
            false,
            Some(42),
            Some(42)
        ));
        assert!(!may_use_wechat_compatibility(
            true,
            true,
            Some(42),
            Some(42)
        ));
        assert!(!may_use_wechat_compatibility(
            true,
            false,
            Some(42),
            Some(43)
        ));
        assert!(!may_use_wechat_compatibility(true, false, None, Some(42)));
        assert!(!may_use_wechat_compatibility(true, false, Some(42), None));
        assert!(!may_use_wechat_compatibility(true, false, Some(0), Some(0)));
    }

    #[test]
    fn qq_group_editor_is_a_candidate_but_a_button_or_window_is_not() {
        assert!(is_edit_container_kind(50026));
        assert!(is_edit_container_kind(50004));
        assert!(!is_edit_container_kind(50000));
        assert!(!is_edit_container_kind(50032));
        // The kind is only admission to pattern inspection, never writable proof.
        assert!(!is_writable_uia_edit(None, None, false));
    }

    #[test]
    fn legacy_editor_requires_text_role_focus_and_writable_unprotected_state() {
        let focused_focusable = 0x4 | 0x100000;
        assert!(is_writable_legacy_text(42, focused_focusable));
        for denied in [0x1, 0x40, 0x8000, 0x10000, 0x20000000] {
            assert!(!is_writable_legacy_text(42, focused_focusable | denied));
        }
        assert!(!is_writable_legacy_text(42, 0x100000));
        assert!(!is_writable_legacy_text(42, 0x4));
        assert!(!is_writable_legacy_text(10, focused_focusable));
    }

    #[test]
    fn multiline_text_pattern_can_prove_writability_without_value_or_text_edit() {
        assert!(is_writable_uia_edit(None, Some(false), false));
        assert!(!is_writable_uia_edit(None, None, false));
    }

    #[test]
    fn explicit_read_only_evidence_overrides_other_writable_patterns() {
        assert!(!is_writable_uia_edit(None, Some(true), true));
        assert!(!is_writable_uia_edit(Some(false), Some(true), false));
        assert!(!is_writable_uia_edit(Some(true), Some(false), true));
        assert!(is_writable_uia_edit(Some(false), None, false));
        assert!(is_writable_uia_edit(None, None, true));
    }

    #[test]
    fn paste_requires_same_window_and_edit_target() {
        let captured = PasteTarget {
            hwnd: 42,
            identity: EditIdentity::Uia(vec![1, 7, 3]),
        };
        assert!(may_paste(
            &captured,
            42,
            Some(&EditIdentity::Uia(vec![1, 7, 3]))
        ));
        assert!(!may_paste(
            &captured,
            99,
            Some(&EditIdentity::Uia(vec![1, 7, 3]))
        ));
        assert!(!may_paste(
            &captured,
            42,
            Some(&EditIdentity::Uia(vec![1, 7, 4]))
        ));
        assert!(!may_paste(&captured, 42, None));
    }

    #[test]
    fn restore_only_accepts_the_original_window_or_our_picker_as_foreground() {
        assert!(may_restore(42, 7, 42, 7, 99));
        assert!(may_restore(42, 7, 88, 99, 99));
        assert!(!may_restore(42, 7, 77, 8, 99));
        assert!(!may_restore(42, 7, 43, 7, 99));
        assert!(!may_restore(42, 99, 88, 99, 99));
    }
}
