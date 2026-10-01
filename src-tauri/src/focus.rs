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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PasteTarget {
    hwnd: isize,
    runtime_id: Vec<i32>,
}

fn may_paste(target: &PasteTarget, foreground_hwnd: isize, focused_id: Option<&[i32]>) -> bool {
    target.hwnd == foreground_hwnd && focused_id == Some(target.runtime_id.as_slice())
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
        let paste_target = windows_impl::focused_edit_id().map(|runtime_id| PasteTarget {
            hwnd: hwnd.0 as isize,
            runtime_id,
        });
        Some(CapturedFocus { hwnd: hwnd.0 as isize, pid, paste_target })
    }
}

#[cfg(not(windows))]
pub fn capture() -> Option<CapturedFocus> {
    None
}

#[cfg(windows)]
mod windows_impl {
    use super::{may_paste, CapturedFocus};
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
        let ctx = Box::new(EnumCtx { pid, found: Mutex::new(None) });
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
            let target = if IsWindow(Some(hwnd)).as_bool() {
                Some(hwnd)
            } else {
                find_window_by_pid(focus.pid) // HWND 失效 → pid 兜底
            };
            let Some(target) = target else { return };

            // 前台锁定绕行：把当前线程短暂附加到前台线程与目标线程
            let fg = GetForegroundWindow();
            let this_thread = GetCurrentThreadId();
            let fg_thread = if fg.0.is_null() { 0 } else { GetWindowThreadProcessId(fg, None) };
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

    /// 读取当前 UIA 焦点：仅接受有键盘焦点、已启用、非密码的可编辑控件。
    /// 对没有可靠 UIA 身份的应用，调用方保留剪贴板供手工粘贴。
    pub fn focused_edit_id() -> Option<Vec<i32>> {
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
        use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
        use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationValuePattern, UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_ValuePatternId};

        unsafe {
            let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let result = (|| {
                let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
                let element = automation.GetFocusedElement().ok()?;
                if !element.CurrentHasKeyboardFocus().ok()?.as_bool()
                    || !element.CurrentIsKeyboardFocusable().ok()?.as_bool()
                    || !element.CurrentIsEnabled().ok()?.as_bool()
                    || element.CurrentIsPassword().ok()?.as_bool()
                {
                    return None;
                }
                let kind = element.CurrentControlType().ok()?;
                if kind != UIA_EditControlTypeId && kind != UIA_DocumentControlTypeId {
                    return None;
                }
                let value = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId);
                match value {
                    Ok(pattern) if pattern.CurrentIsReadOnly().ok()?.as_bool() => return None,
                    Err(_) if kind == UIA_DocumentControlTypeId => return None,
                    _ => {}
                }
                let array = element.GetRuntimeId().ok()?;
                if array.is_null() { return None; }
                let ids = (|| {
                    if SafeArrayGetDim(array) != 1 { return None; }
                    let lower = SafeArrayGetLBound(array, 1).ok()?;
                    let upper = SafeArrayGetUBound(array, 1).ok()?;
                    if upper < lower || upper - lower > 128 { return None; }
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
            if initialized { CoUninitialize(); }
            result
        }
    }

    pub fn paste_if_editable(focus: Option<&CapturedFocus>) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V};
        let Some(focus) = focus else { return false };
        if focus.pid == std::process::id() { return false; }
        let Some(target) = &focus.paste_target else { return false };
        let foreground = unsafe { GetForegroundWindow() };
        let foreground_hwnd = foreground.0 as isize;
        if !may_paste(target, foreground_hwnd, focused_edit_id().as_deref()) { return false; }
        // 用户还按着快捷键修饰键时不注入，避免组合成其他操作。
        for key in [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN] {
            if unsafe { GetAsyncKeyState(key.0 as i32) } < 0 { return false; }
        }
        let key = |vk, up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT {
                wVk: vk,
                dwFlags: if up { KEYEVENTF_KEYUP } else { Default::default() },
                ..Default::default()
            } },
        };
        // 最后再确认前台没变；SendInput 仍可能被 UIPI 拦截，失败时剪贴板可手工粘贴。
        if unsafe { GetForegroundWindow() }.0 as isize != target.hwnd { return false; }
        let inputs = [key(VK_CONTROL, false), key(VK_V, false), key(VK_V, true), key(VK_CONTROL, true)];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 && sent > 0 {
            // 若仅注入了前缀，补发释放事件，避免 Ctrl 卡在按下状态。
            let release = [key(VK_V, true), key(VK_CONTROL, true)];
            unsafe { SendInput(&release, std::mem::size_of::<INPUT>() as i32); }
        }
        sent == inputs.len() as u32
    }
}

#[cfg(windows)]
pub use windows_impl::{paste_if_editable, restore};

#[cfg(not(windows))]
pub fn restore(_focus: Option<&CapturedFocus>) {}

#[cfg(not(windows))]
pub fn paste_if_editable(_focus: Option<&CapturedFocus>) -> bool { false }

#[cfg(test)]
mod paste_tests {
    use super::*;

    #[test]
    fn paste_requires_same_window_and_edit_target() {
        let captured = PasteTarget { hwnd: 42, runtime_id: vec![1, 7, 3] };
        assert!(may_paste(&captured, 42, Some(&[1, 7, 3])));
        assert!(!may_paste(&captured, 99, Some(&[1, 7, 3])));
        assert!(!may_paste(&captured, 42, Some(&[1, 7, 4])));
        assert!(!may_paste(&captured, 42, None));
    }
}
