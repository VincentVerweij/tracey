use super::{PlatformHooks, WindowInfo};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

pub struct WindowsPlatformHooks;

impl PlatformHooks for WindowsPlatformHooks {
    fn get_foreground_window_info(&self) -> Option<WindowInfo> {
        unsafe {
            // HWND null check: compare inner pointer with null_mut().
            // In windows crate 0.58+, HWND wraps a raw pointer — do NOT compare == 0.
            let hwnd = GetForegroundWindow();
            if hwnd == HWND(std::ptr::null_mut()) {
                return None;
            }

            // Get window title
            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

            // Get process ID
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }

            // Open process to query executable path
            let process = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                pid,
            )
            .ok()?;

            let mut path_buf = [0u16; 1024];
            let path_len = GetModuleFileNameExW(process, None, &mut path_buf);
            let process_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);

            let process_name = std::path::Path::new(&process_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();

            Some(WindowInfo {
                title,
                process_name,
                process_path,
            })
        }
    }

    fn get_idle_seconds(&self) -> u64 {
        unsafe {
            let mut last_input = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };

            if GetLastInputInfo(&mut last_input).as_bool() {
                // GetTickCount64 MUST be used (not GetTickCount)
                // GetTickCount wraps at ~49 days; GetTickCount64 does not
                let now = GetTickCount64();
                let elapsed_ms = now.saturating_sub(last_input.dwTime as u64);
                elapsed_ms / 1000
            } else {
                0
            }
        }
    }
}
