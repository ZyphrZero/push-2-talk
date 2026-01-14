// Windows 原生键盘输入模块
// 使用 Win32 SendInput API 替代跨平台 enigo 库
// 提供更低延迟的键盘模拟功能

use anyhow::Result;
use std::thread;
use std::time::Duration;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
    VK_SHIFT, VK_LSHIFT, VK_RSHIFT,
    VK_MENU, VK_LMENU, VK_RMENU,
    VK_LWIN, VK_RWIN,
    VK_C, VK_V,
};

/// 按键间延迟（毫秒）
/// 保守设置以确保在各种应用中稳定工作
const KEY_DELAY_MS: u64 = 15;

/// 发送单个按键按下事件
#[cfg(target_os = "windows")]
fn send_key_down(vk: VIRTUAL_KEY) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let result = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    if result == 0 {
        anyhow::bail!("SendInput failed for key down: {:?}", vk);
    }

    Ok(())
}

/// 发送单个按键释放事件
#[cfg(target_os = "windows")]
fn send_key_up(vk: VIRTUAL_KEY) -> Result<()> {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let result = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };

    if result == 0 {
        anyhow::bail!("SendInput failed for key up: {:?}", vk);
    }

    Ok(())
}

/// 模拟 Ctrl+C 组合键（复制）
#[cfg(target_os = "windows")]
pub fn send_ctrl_c() -> Result<()> {
    tracing::debug!("win32_input: 发送 Ctrl+C");

    // 按下 Ctrl
    send_key_down(VK_CONTROL)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 按下并释放 C
    send_key_down(VK_C)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    send_key_up(VK_C)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 释放 Ctrl
    send_key_up(VK_CONTROL)?;

    Ok(())
}

/// 模拟 Ctrl+V 组合键（粘贴）
#[cfg(target_os = "windows")]
pub fn send_ctrl_v() -> Result<()> {
    tracing::debug!("win32_input: 发送 Ctrl+V");

    // 按下 Ctrl
    send_key_down(VK_CONTROL)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 按下并释放 V
    send_key_down(VK_V)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));
    send_key_up(VK_V)?;
    thread::sleep(Duration::from_millis(KEY_DELAY_MS));

    // 释放 Ctrl
    send_key_up(VK_CONTROL)?;

    Ok(())
}

/// 释放所有修饰键（防御性措施）
/// 用于确保热键释放后不会有残留的修饰键状态
#[cfg(target_os = "windows")]
pub fn release_all_modifiers() -> Result<()> {
    tracing::debug!("win32_input: 释放所有修饰键");

    // 尝试释放所有可能的修饰键，忽略错误
    // 同时释放通用键码和左右分开的键码，确保完全清除
    let _ = send_key_up(VK_CONTROL);
    let _ = send_key_up(VK_LCONTROL);
    let _ = send_key_up(VK_RCONTROL);
    let _ = send_key_up(VK_SHIFT);
    let _ = send_key_up(VK_LSHIFT);
    let _ = send_key_up(VK_RSHIFT);
    let _ = send_key_up(VK_MENU);    // Alt
    let _ = send_key_up(VK_LMENU);   // 左 Alt
    let _ = send_key_up(VK_RMENU);   // 右 Alt
    let _ = send_key_up(VK_LWIN);    // 左 Windows 键
    let _ = send_key_up(VK_RWIN);    // 右 Windows 键

    Ok(())
}

// 非 Windows 平台的空实现（编译占位）
#[cfg(not(target_os = "windows"))]
pub fn send_ctrl_c() -> Result<()> {
    anyhow::bail!("win32_input: 仅支持 Windows 平台")
}

#[cfg(not(target_os = "windows"))]
pub fn send_ctrl_v() -> Result<()> {
    anyhow::bail!("win32_input: 仅支持 Windows 平台")
}

#[cfg(not(target_os = "windows"))]
pub fn release_all_modifiers() -> Result<()> {
    anyhow::bail!("win32_input: 仅支持 Windows 平台")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_send_ctrl_c() {
        // 注意：此测试会实际发送键盘事件
        // 仅在开发环境手动运行
        // let result = send_ctrl_c();
        // assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_send_ctrl_v() {
        // 注意：此测试会实际发送键盘事件
        // 仅在开发环境手动运行
        // let result = send_ctrl_v();
        // assert!(result.is_ok());
    }
}
