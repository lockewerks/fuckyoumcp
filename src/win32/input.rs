//! # Mouse & Keyboard Input via SendInput
//!
//! The Win32 input injection API: `SendInput`. One function to rule them all.
//! It takes an array of INPUT structs, each describing either a mouse event,
//! keyboard event, or hardware event. We use it for everything: moving the
//! mouse, clicking, scrolling, typing text, pressing key combos.
//!
//! For typing arbitrary Unicode text, we use KEYEVENTF_UNICODE which lets us
//! send characters directly without mapping them to virtual key codes. This
//! means we can type emoji, CJK characters, or anything else without caring
//! about the keyboard layout. The future is now.
//!
//! For key combos (Ctrl+C, Alt+Tab, etc.), we map key names to virtual key
//! codes and send the appropriate down/up sequences.

use super::pretty;
use anyhow::Result;
use serde_json::json;
use std::mem::size_of;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Get the current cursor position as (x, y) screen coordinates.
pub fn cursor_position() -> Result<String> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt)?;
        Ok(pretty(&json!({
            "X": pt.x,
            "Y": pt.y,
        })))
    }
}

/// Move the cursor to absolute screen coordinates.
pub fn mouse_move(x: i32, y: i32) -> Result<String> {
    unsafe {
        SetCursorPos(x, y)?;
        Ok(pretty(&json!({
            "Status": "Moved",
            "X": x,
            "Y": y,
        })))
    }
}

/// Click a mouse button at optional coordinates.
/// button: "left", "right", "middle" (default: "left")
/// count: 1 = single click, 2 = double click (default: 1)
pub fn mouse_click(
    x: Option<i32>,
    y: Option<i32>,
    button: &str,
    count: u32,
) -> Result<String> {
    unsafe {
        // Move to position first if coordinates specified
        if let (Some(cx), Some(cy)) = (x, y) {
            SetCursorPos(cx, cy)?;
            // Brief pause so target app registers the cursor position
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let (down, up) = button_flags(button);
        let click_count = count.max(1).min(5);

        for _ in 0..click_count {
            let inputs = [mouse_input(down), mouse_input(up)];
            SendInput(&inputs, size_of::<INPUT>() as i32);
            // Small delay between clicks for double/triple click detection
            if click_count > 1 {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        }

        // Get final position for reporting
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        Ok(pretty(&json!({
            "Status": "Clicked",
            "Button": button,
            "Count": click_count,
            "X": pt.x,
            "Y": pt.y,
        })))
    }
}

/// Scroll the mouse wheel. Positive = up, negative = down.
/// Each unit is one wheel click (WHEEL_DELTA = 120).
pub fn mouse_scroll(
    x: Option<i32>,
    y: Option<i32>,
    clicks: i32,
) -> Result<String> {
    unsafe {
        if let (Some(cx), Some(cy)) = (x, y) {
            SetCursorPos(cx, cy)?;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        const WHEEL_DELTA: i32 = 120;
        let amount = clicks * WHEEL_DELTA;

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: amount as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        SendInput(&[input], size_of::<INPUT>() as i32);

        Ok(pretty(&json!({
            "Status": "Scrolled",
            "Clicks": clicks,
            "Direction": if clicks > 0 { "Up" } else { "Down" },
        })))
    }
}

/// Drag from (start_x, start_y) to (end_x, end_y) with the specified button.
pub fn mouse_drag(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    button: &str,
) -> Result<String> {
    unsafe {
        let (down, up) = button_flags(button);

        // Move to start position
        SetCursorPos(start_x, start_y)?;
        std::thread::sleep(std::time::Duration::from_millis(15));

        // Press button down
        SendInput(&[mouse_input(down)], size_of::<INPUT>() as i32);
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Move to end position — use multiple intermediate steps for smooth drag
        let steps = 10;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let ix = start_x + ((end_x - start_x) as f64 * t) as i32;
            let iy = start_y + ((end_y - start_y) as f64 * t) as i32;
            SetCursorPos(ix, iy)?;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        std::thread::sleep(std::time::Duration::from_millis(15));

        // Release button
        SendInput(&[mouse_input(up)], size_of::<INPUT>() as i32);

        Ok(pretty(&json!({
            "Status": "Dragged",
            "Button": button,
            "From": { "X": start_x, "Y": start_y },
            "To": { "X": end_x, "Y": end_y },
        })))
    }
}

/// Type arbitrary Unicode text by injecting KEYEVENTF_UNICODE events.
/// Works with any character including emoji, CJK, etc. regardless of
/// keyboard layout. Each character gets a key-down then key-up event.
pub fn keyboard_type(text: &str) -> Result<String> {
    let mut count = 0u32;
    for ch in text.encode_utf16() {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: ch,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: ch,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        unsafe {
            SendInput(&inputs, size_of::<INPUT>() as i32);
        }
        count += 1;
    }

    Ok(pretty(&json!({
        "Status": "Typed",
        "Characters": count,
    })))
}

/// Press a key combination like "ctrl+c", "alt+tab", "shift+f5", "enter".
/// Multiple keys separated by "+" are pressed in order (modifiers first),
/// then released in reverse order (modifiers last).
///
/// Supported key names:
///   Modifiers: ctrl, shift, alt, win
///   Navigation: up, down, left, right, home, end, pageup, pagedown
///   Editing: enter, tab, escape, backspace, delete, insert, space
///   Function: f1-f24
///   Letters: a-z
///   Numbers: 0-9
///   Media: printscreen, scrolllock, pause, numlock, capslock
pub fn keyboard_key(keys: &str) -> Result<String> {
    let parts: Vec<&str> = keys.split('+').map(|s| s.trim()).collect();
    let mut vks: Vec<u16> = Vec::new();

    for part in &parts {
        match vk_from_name(part) {
            Some(vk) => vks.push(vk),
            None => anyhow::bail!("Unknown key name: '{part}'"),
        }
    }

    if vks.is_empty() {
        anyhow::bail!("No keys specified");
    }

    // Build input sequence: all keys down, then all keys up (reverse order)
    let mut inputs: Vec<INPUT> = Vec::with_capacity(vks.len() * 2);

    // Key down events — in order
    for &vk in &vks {
        inputs.push(key_input(vk, false));
    }

    // Key up events — reverse order (release main key first, then modifiers)
    for &vk in vks.iter().rev() {
        inputs.push(key_input(vk, true));
    }

    unsafe {
        let sent = SendInput(&inputs, size_of::<INPUT>() as i32);
        if sent == 0 {
            anyhow::bail!("SendInput failed — no events were injected");
        }
    }

    Ok(pretty(&json!({
        "Status": "Pressed",
        "Keys": keys,
        "EventsSent": vks.len() * 2,
    })))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Map a key name to its virtual key code.
fn vk_from_name(name: &str) -> Option<u16> {
    let lower = name.to_lowercase();

    // Check for F-keys first (f1 through f24)
    if lower.starts_with('f') && lower.len() >= 2 {
        if let Ok(n) = lower[1..].parse::<u16>() {
            if (1..=24).contains(&n) {
                return Some(0x6F + n); // VK_F1=0x70, VK_F2=0x71, ...
            }
        }
    }

    match lower.as_str() {
        // Modifiers
        "ctrl" | "control" => Some(0x11),
        "shift" => Some(0x10),
        "alt" | "menu" => Some(0x12),
        "win" | "windows" | "super" | "meta" | "cmd" => Some(0x5B),

        // Navigation
        "up" => Some(0x26),
        "down" => Some(0x28),
        "left" => Some(0x25),
        "right" => Some(0x27),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" | "pgup" => Some(0x21),
        "pagedown" | "pgdn" => Some(0x22),

        // Editing
        "enter" | "return" => Some(0x0D),
        "tab" => Some(0x09),
        "escape" | "esc" => Some(0x1B),
        "backspace" | "back" => Some(0x08),
        "delete" | "del" => Some(0x2E),
        "insert" | "ins" => Some(0x2D),
        "space" => Some(0x20),

        // Toggle/Lock
        "capslock" | "caps" => Some(0x14),
        "numlock" => Some(0x90),
        "scrolllock" => Some(0x91),

        // System
        "printscreen" | "prtsc" | "print" => Some(0x2C),
        "pause" | "break" => Some(0x13),
        "apps" | "contextmenu" => Some(0x5D),

        // Single character: letter or digit
        s if s.len() == 1 => {
            let ch = s.chars().next()?;
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_uppercase() as u16)
            } else {
                // OEM keys
                match ch {
                    ';' | ':' => Some(0xBA),
                    '=' | '+' => Some(0xBB),
                    ',' | '<' => Some(0xBC),
                    '-' | '_' => Some(0xBD),
                    '.' | '>' => Some(0xBE),
                    '/' | '?' => Some(0xBF),
                    '`' | '~' => Some(0xC0),
                    '[' | '{' => Some(0xDB),
                    '\\' | '|' => Some(0xDC),
                    ']' | '}' => Some(0xDD),
                    '\'' | '"' => Some(0xDE),
                    _ => None,
                }
            }
        }

        _ => None,
    }
}

/// Get the MOUSE_EVENT_FLAGS for button down/up.
fn button_flags(button: &str) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
    match button.to_lowercase().as_str() {
        "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
    }
}

/// Create a MOUSEINPUT INPUT event with the given flags.
fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Create a KEYBDINPUT INPUT event for a virtual key code.
fn key_input(vk: u16, key_up: bool) -> INPUT {
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
