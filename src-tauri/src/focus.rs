//! 焦点捕获与恢复：呼出前记录前台应用，复制后把焦点还给呼出方。
//!
//! Seam：`capture` / `restore`。
//!
//! Windows：记录 `GetForegroundWindow()` 的 HWND + pid；恢复 = SetForegroundWindow
//! （前台锁定时 AttachThreadInput 绕行），HWND 失效时按 pid 枚举剩余可见窗口兜底。
//! macOS：记录 frontmostApplication（bundle id + pid）恢复 activate —— 随打包票落地。
//! 自动粘贴为 P1 功能：Windows 使用只读 UI Automation 查询焦点控件身份；
//! 焦点恢复本身仍不依赖辅助功能 API。识别失败时保留复制结果供手工粘贴。

#[derive(Debug)]
pub struct CapturedFocus {
    /// Windows：HWND；其他平台：0
    hwnd: isize,
    /// 前台窗口所属进程（HWND 失效时的兜底索引），0 表示未知
    pid: u32,
    /// UI Automation 的可编辑控件身份；不可确认时只复制，不注入粘贴。
    paste_target: Option<PasteTarget>,
}

#[derive(Debug, Clone, Copy)]
pub enum PasteFailure {
    NoOriginalWindow,
    NoEditableTarget,
    WindowChanged,
    InputChanged,
    ModifierHeld,
    SystemRejected,
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
        }
    }
}

pub fn has_editable_target(focus: Option<&CapturedFocus>) -> bool {
    focus.is_some_and(|value| value.paste_target.is_some())
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

/// 捕获当前前台窗口（应在呼出 Quick Picker **之前**调用）。
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
        if pid == 0 {
            return None;
        }
        let paste_target =
            windows_impl::focused_edit_target(hwnd, pid).map(|identity| PasteTarget {
                hwnd: hwnd.0 as isize,
                identity,
            });
        Some(CapturedFocus {
            hwnd: hwnd.0 as isize,
            pid,
            paste_target,
        })
    }
}

#[cfg(not(windows))]
pub fn capture() -> Option<CapturedFocus> {
    None
}

#[cfg(windows)]
mod windows_impl {
    use super::{may_paste, CapturedFocus, EditIdentity};
    use std::sync::Mutex;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
        IsWindowVisible, SetForegroundWindow,
    };

    struct EnumCtx {
        pid: u32,
        found: Mutex<Option<HWND>>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &*(lparam.0 as *const EnumCtx) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == ctx.pid {
            let visible = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
            if visible {
                if let Ok(mut slot) = ctx.found.lock() {
                    if slot.is_none() {
                        *slot = Some(hwnd);
                        return BOOL(0); // 找到即停
                    }
                }
            }
        }
        BOOL(1)
    }

    fn find_window_by_pid(pid: u32) -> Option<HWND> {
        let ctx = Box::new(EnumCtx {
            pid,
            found: Mutex::new(None),
        });
        let lparam = LPARAM(&*ctx as *const EnumCtx as isize);
        unsafe {
            let _ = EnumWindows(Some(enum_proc), lparam);
        }
        let found = ctx.found.lock().ok().and_then(|slot| *slot);
        found
    }

    pub fn restore(focus: Option<&CapturedFocus>) {
        let Some(focus) = focus else { return };
        if focus.pid == std::process::id() {
            return; // 呼出方是自己（异常情况），不做恢复
        }
        unsafe {
            let hwnd = HWND(focus.hwnd as *mut _);
            let mut current_pid = 0u32;
            if IsWindow(Some(hwnd)).as_bool() {
                GetWindowThreadProcessId(hwnd, Some(&mut current_pid));
            }
            let target = if current_pid == focus.pid {
                Some(hwnd)
            } else {
                find_window_by_pid(focus.pid) // HWND 失效 → pid 兜底
            };
            let Some(target) = target else { return };

            // 前台锁定绕行：把当前线程短暂附加到前台线程与目标线程
            let fg = GetForegroundWindow();
            let this_thread = GetCurrentThreadId();
            let fg_thread = if fg.0.is_null() {
                0
            } else {
                GetWindowThreadProcessId(fg, None)
            };
            let target_thread = GetWindowThreadProcessId(target, None);

            let attached_fg = fg_thread != 0 && fg_thread != this_thread;
            if attached_fg {
                let _ = AttachThreadInput(this_thread, fg_thread, true);
            }
            let attached_target = target_thread != 0 && target_thread != this_thread;
            if attached_target {
                let _ = AttachThreadInput(this_thread, target_thread, true);
            }
            let _ = SetForegroundWindow(target);
            if attached_fg {
                let _ = AttachThreadInput(this_thread, fg_thread, false);
            }
            if attached_target {
                let _ = AttachThreadInput(this_thread, target_thread, false);
            }
        }
    }

    /// 只接受同一前台窗口中可证明可编辑的 UIA 控件或标准 Win32 编辑框。
    pub fn focused_edit_target(hwnd: HWND, pid: u32) -> Option<EditIdentity> {
        if unsafe { GetForegroundWindow() } != hwnd {
            return None;
        }
        focused_native_edit(hwnd, pid)
            .map(EditIdentity::Native)
            .or_else(|| focused_uia_edit_id().map(EditIdentity::Uia))
            .filter(|_| unsafe { GetForegroundWindow() } == hwnd)
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

    fn focused_uia_edit_id() -> Option<Vec<i32>> {
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::System::Ole::{
            SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound,
            SafeArrayGetUBound,
        };
        use windows::Win32::UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextEditPattern,
            IUIAutomationValuePattern, UIA_CustomControlTypeId, UIA_DocumentControlTypeId,
            UIA_EditControlTypeId, UIA_PaneControlTypeId, UIA_TextControlTypeId,
            UIA_TextEditPatternId, UIA_ValuePatternId,
        };

        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let result = (|| {
                let automation: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                let focused = automation.GetFocusedElement().ok()?;
                if !focused.CurrentHasKeyboardFocus().ok()?.as_bool()
                    || !focused.CurrentIsKeyboardFocusable().ok()?.as_bool()
                    || !focused.CurrentIsEnabled().ok()?.as_bool()
                    || focused.CurrentIsPassword().ok()?.as_bool()
                {
                    return None;
                }
                let mut element: IUIAutomationElement = focused.clone();
                let walker = automation.ControlViewWalker().ok()?;
                let mut proven_edit = false;
                for depth in 0..=4 {
                    let kind = element.CurrentControlType().ok()?;
                    if kind == UIA_EditControlTypeId || kind == UIA_DocumentControlTypeId {
                        let value = element
                            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId);
                        proven_edit = match value {
                            Ok(pattern) => !pattern.CurrentIsReadOnly().ok()?.as_bool(),
                            Err(_) => element
                                .GetCurrentPatternAs::<IUIAutomationTextEditPattern>(
                                    UIA_TextEditPatternId,
                                )
                                .is_ok(),
                        };
                        if proven_edit {
                            break;
                        }
                    }
                    if depth == 4
                        || ![
                            UIA_TextControlTypeId,
                            UIA_CustomControlTypeId,
                            UIA_PaneControlTypeId,
                        ]
                        .contains(&kind)
                    {
                        break;
                    }
                    element = walker.GetParentElement(&element).ok()?;
                    if !element.CurrentIsEnabled().ok()?.as_bool()
                        || element.CurrentIsPassword().ok()?.as_bool()
                    {
                        break;
                    }
                }
                if !proven_edit {
                    return None;
                }
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
                    if upper < lower || upper - lower > 128 {
                        return None;
                    }
                    let mut values = Vec::with_capacity((upper - lower + 1) as usize);
                    for index in lower..=upper {
                        let mut value = 0i32;
                        SafeArrayGetElement(array, &index, &mut value as *mut _ as *mut _).ok()?;
                        values.push(value);
                    }
                    Some(values)
                })();
                let _ = SafeArrayDestroy(array);
                ids
            })();
            if initialized {
                CoUninitialize();
            }
            result
        }
    }

    pub fn paste_if_editable(focus: Option<&CapturedFocus>) -> Result<(), super::PasteFailure> {
        use super::PasteFailure;
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
        let Some(target) = &focus.paste_target else {
            return Err(PasteFailure::NoEditableTarget);
        };
        let foreground = unsafe { GetForegroundWindow() };
        let foreground_hwnd = foreground.0 as isize;
        if foreground_hwnd != target.hwnd {
            return Err(PasteFailure::WindowChanged);
        }
        if !may_paste(
            target,
            foreground_hwnd,
            focused_edit_target(foreground, focus.pid).as_ref(),
        ) {
            return Err(PasteFailure::InputChanged);
        }
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
        if unsafe { GetForegroundWindow() }.0 as isize != target.hwnd {
            return Err(PasteFailure::WindowChanged);
        }
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 && sent > 0 {
            // 若仅注入了前缀，补发释放事件，避免 Ctrl 卡在按下状态。
            let release = [key(VK_V, true), key(VK_CONTROL, true)];
            unsafe {
                SendInput(&release, std::mem::size_of::<INPUT>() as i32);
            }
        }
        if sent == inputs.len() as u32 {
            Ok(())
        } else {
            Err(PasteFailure::SystemRejected)
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{paste_if_editable, restore};

#[cfg(not(windows))]
pub fn restore(_focus: Option<&CapturedFocus>) {}

#[cfg(not(windows))]
pub fn paste_if_editable(_focus: Option<&CapturedFocus>) -> Result<(), PasteFailure> {
    Err(PasteFailure::NoEditableTarget)
}

#[cfg(test)]
mod paste_tests {
    use super::*;

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
}
