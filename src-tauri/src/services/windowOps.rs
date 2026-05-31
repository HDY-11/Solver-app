// services/windowOps.rs — Windows API 窗口贴靠操作

use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_LEFT, VK_RIGHT, VK_UP, VK_LWIN,
};

fn snap_vk(vk: u8) {
    unsafe {
        keybd_event(VK_LWIN.0 as u8, 0, KEYBD_EVENT_FLAGS::default(), 0);
        keybd_event(vk, 0, KEYBD_EVENT_FLAGS::default(), 0);
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }
}

/// Win+← 贴靠到左半屏
pub fn snap_left() { snap_vk(VK_LEFT.0 as u8); }
/// Win+→ 贴靠到右半屏
pub fn snap_right() { snap_vk(VK_RIGHT.0 as u8); }
/// Win+↑ 最大化
pub fn snap_maximize() { snap_vk(VK_UP.0 as u8); }

