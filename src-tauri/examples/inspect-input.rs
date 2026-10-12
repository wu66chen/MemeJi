// Read-only support tool. Never captures input text or sends keyboard events.
#[cfg(windows)]
#[allow(dead_code, unused_imports)]
#[path = "../src/focus.rs"]
mod focus;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MemeJi 输入框诊断：请在 8 秒内点击要测试的 QQ / 微信输入框，并保持焦点。\n此工具不会读取聊天内容，也不会发送按键。");
    for seconds in (1..=8).rev() {
        println!("{seconds}…");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    let captured = focus::enrich_for_paste(focus::capture());
    let mut metadata = String::new();
    for sample in 0..3 {
        metadata.push_str(&format!(
            "\n--- Metadata sample {sample} ---\n{}",
            inspect_metadata()
        ));
        if sample < 2 {
            std::thread::sleep(std::time::Duration::from_millis(750));
        }
    }
    let report = format!(
        "MemeJi input diagnostic v3 (UIA Group + MSAA focus)\nEditable target: {}\nResult: {}\nCapture: {captured:#?}\n{metadata}\nNo input text, window title or clipboard content was read.\n",
        focus::has_editable_target(captured.as_ref()),
        focus::input_diagnostic(captured.as_ref()),
    );
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let output =
        std::env::current_exe()?.with_file_name(format!("MemeJi-input-diagnostic-{timestamp}.txt"));
    std::fs::write(&output, &report)?;
    println!(
        "\n{report}\n诊断文件：{}\n按 Enter 关闭。",
        output.display()
    );
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(())
}

// Deliberately omit UIA Name/Value, AutomationId, text ranges and window titles.
// Inspect focused elements and their ancestors only, never chat descendants.
#[cfg(windows)]
fn inspect_metadata() -> String {
    use std::fmt::Write;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe fn native_metadata(hwnd: HWND) -> String {
        let mut class = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class) };
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        format!(
            "hwnd={:?}, pid={pid}, thread={thread}, class={}, style={:#x}",
            hwnd.0,
            String::from_utf16_lossy(&class[..len.max(0) as usize]),
            unsafe { GetWindowLongW(hwnd, GWL_STYLE) }
        )
    }
    unsafe fn element_metadata(element: &IUIAutomationElement) -> String {
        // HRESULTs are retained to distinguish unsupported providers from access failure.
        unsafe {
            let value = element
                .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                .and_then(|pattern| pattern.CurrentIsReadOnly());
            let text = element
                .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .and_then(|pattern| pattern.DocumentRange())
                .and_then(|range| range.GetAttributeValue(UIA_IsReadOnlyAttributeId))
                .map(|value| (value.Anonymous.Anonymous.vt, bool::try_from(&value).ok()));
            let legacy = element
                .GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
                    UIA_LegacyIAccessiblePatternId,
                )
                .map(|pattern| (pattern.CurrentRole(), pattern.CurrentState()));
            format!("type={:?}, pid={:?}, native={:?}, framework={:?}, class={:?}, keyboardFocus={:?}, focusable={:?}, enabled={:?}, password={:?}, ValueReadOnly={value:?}, TextReadOnly={text:?}, TextEdit={:?}, LegacyRoleState={legacy:?}",
                element.CurrentControlType(), element.CurrentProcessId(), element.CurrentNativeWindowHandle(),
                element.CurrentFrameworkId(), element.CurrentClassName(), element.CurrentHasKeyboardFocus(),
                element.CurrentIsKeyboardFocusable(), element.CurrentIsEnabled(), element.CurrentIsPassword(),
                element.GetCurrentPatternAs::<IUIAutomationTextEditPattern>(UIA_TextEditPatternId).map(|_| ()))
        }
    }
    let mut report = String::new();
    unsafe {
        let foreground = GetForegroundWindow();
        let _ = writeln!(report, "Foreground: {}", native_metadata(foreground));
        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        let gui_result = GetGUIThreadInfo(thread, &mut info);
        let _ = writeln!(report, "GUIThread: {gui_result:?}, flags={:?}, active={:?}, focus={:?}, caret={:?}, caretRect={:?}", info.flags, info.hwndActive, info.hwndFocus, info.hwndCaret, info.rcCaret);
        if gui_result.is_ok() {
            let _ = writeln!(report, "Native focus: {}", native_metadata(info.hwndFocus));
            if !info.hwndCaret.0.is_null() {
                let _ = writeln!(report, "Native caret: {}", native_metadata(info.hwndCaret));
            }
        }
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let automation: windows::core::Result<IUIAutomation> =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER);
        match automation {
            Err(error) => {
                let _ = writeln!(report, "UIA init: {error:?}");
            }
            Ok(automation) => {
                match automation.GetFocusedElement() {
                    Err(error) => {
                        let _ = writeln!(report, "UIA focus: {error:?}");
                    }
                    Ok(mut element) => {
                        for depth in 0..6 {
                            let _ = writeln!(
                                report,
                                "UIA focused/ancestor {depth}: {}",
                                element_metadata(&element)
                            );
                            let parent = automation
                                .RawViewWalker()
                                .and_then(|walker| walker.GetParentElement(&element));
                            match parent {
                                Ok(parent) => element = parent,
                                Err(error) => {
                                    let _ = writeln!(report, "UIA parent: {error:?}");
                                    break;
                                }
                            }
                        }
                    }
                }
                if gui_result.is_ok() && !info.hwndFocus.0.is_null() {
                    match automation.ElementFromHandle(info.hwndFocus) {
                        Ok(element) => {
                            let _ = writeln!(
                                report,
                                "UIA from native focus: {}",
                                element_metadata(&element)
                            );
                        }
                        Err(error) => {
                            let _ = writeln!(report, "UIA from native focus: {error:?}");
                        }
                    }
                }
                let mut point = windows::Win32::Foundation::POINT::default();
                if GetCursorPos(&mut point).is_ok() {
                    match automation.ElementFromPoint(point) {
                        Ok(mut element) => {
                            for depth in 0..4 {
                                let _ = writeln!(
                                    report,
                                    "UIA cursor/ancestor {depth}: {}",
                                    element_metadata(&element)
                                );
                                match automation
                                    .RawViewWalker()
                                    .and_then(|walker| walker.GetParentElement(&element))
                                {
                                    Ok(parent) => element = parent,
                                    Err(_) => break,
                                }
                            }
                        }
                        Err(error) => {
                            let _ = writeln!(report, "UIA cursor: {error:?}");
                        }
                    }
                }
            }
        }
        if initialized {
            CoUninitialize();
        }
        let _ = writeln!(
            report,
            "Foreground unchanged: {}",
            GetForegroundWindow() == foreground
        );
    }
    report
}

#[cfg(not(windows))]
fn main() {
    println!("This diagnostic tool is for Windows only.");
}
