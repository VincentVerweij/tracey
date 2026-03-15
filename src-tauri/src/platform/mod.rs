/// Platform abstraction for OS-level queries.
/// Windows implementation is in `platform::windows`.
///
/// Architecture decision (2026-03-15): This trait has exactly TWO methods.
/// `trigger_screenshot_capture()` was removed — capture triggering lives in
/// ActivityTracker/ScreenshotService, not in the platform abstraction layer.
pub trait PlatformHooks: Send + Sync {
    /// Returns info about the currently active (foreground) window.
    /// Returns None if no foreground window exists or the query fails.
    fn get_foreground_window_info(&self) -> Option<WindowInfo>;

    /// Returns the number of seconds the system has been idle
    /// (no mouse or keyboard input).
    ///
    /// Implementation uses GetLastInputInfo + GetTickCount64.
    /// GetTickCount64 MUST be used (not GetTickCount) to avoid
    /// 32-bit rollover after ~49 days of uptime.
    fn get_idle_seconds(&self) -> u64;
}

/// Information about the currently active window.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    /// The window title text.
    pub title: String,
    /// The process executable name (e.g. "chrome.exe").
    pub process_name: String,
    /// The full path to the process executable.
    pub process_path: String,
}

#[cfg(target_os = "windows")]
pub mod windows;
