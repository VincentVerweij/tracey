use super::{PlatformHooks, WindowInfo};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

pub mod session_watch;

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
            let process =
                OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;

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
                u64::from(idle_ms(GetTickCount64(), last_input.dwTime)) / 1000
            } else {
                0
            }
        }
    }
}

/// Milliseconds since the last input. `last_input` is `LASTINPUTINFO.dwTime`,
/// a 32-bit tick count that wraps every ~49.7 days of uptime, so the
/// subtraction is done on the low 32 bits of `now` with wrapping arithmetic.
/// Correct across a wrap as long as the user has been idle for less than
/// ~49.7 days.
fn idle_ms(now: u64, last_input: u32) -> u32 {
    (now as u32).wrapping_sub(last_input)
}

#[cfg(test)]
mod tests {
    use super::idle_ms;

    #[test]
    fn idle_is_time_since_last_input() {
        assert_eq!(idle_ms(10_000, 4_000), 6_000);
    }

    /// GetTickCount64 never wraps, but LASTINPUTINFO.dwTime is 32-bit and
    /// wraps to 0 every ~49.7 days of uptime.
    const WRAP: u64 = 1 << 32;

    #[test]
    fn idle_is_correct_after_the_input_tick_wraps() {
        assert_eq!(idle_ms(WRAP + 10_000, 4_000), 6_000);
    }

    #[test]
    fn idle_is_correct_when_input_and_now_straddle_the_wrap() {
        let last_input = (WRAP - 1_000) as u32;
        assert_eq!(idle_ms(WRAP + 5_000, last_input), 6_000);
    }
}
