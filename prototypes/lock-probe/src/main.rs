//! PROTOTYPE — throwaway instrumentation for wayfinder ticket #66 (map #65).
//!
//! Once a second it records, on one line:
//!   - the foreground window (process name + title), exactly as
//!     `PlatformHooks::get_foreground_window_info()` sees it, or why it saw nothing
//!   - the `BitBlt` outcome: OK, ERROR_INVALID_HANDLE (0x80070006), or another HRESULT
//!   - whether the captured frame is actually non-black (mean / max / % zero pixels)
//!   - `get_idle_seconds()`, mirroring the production idle query
//!
//! Capture mirrors `screenshot_service::capture_screen_full_res_jpeg()` up to the
//! pixel buffer, then stops — nothing is encoded or written as an image.
//!
//! Run it, lock the machine, sit on the lock screen for a few minutes, unlock,
//! then Ctrl-C. Every line also lands in `lock-probe.log` next to the exe, because
//! the console is not readable while the machine is locked.
//!
//! No error handling, no abstractions, no tests. It answers one question.

use chrono::Local;
use std::io::Write;
use windows::Win32::Foundation::{GetLastError, HWND};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    GetMonitorInfoW, GetWindowDC, MonitorFromWindow, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    RGBQUAD, SRCCOPY,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

fn foreground() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == HWND(std::ptr::null_mut()) {
            return "fg=NULL".to_string();
        }

        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return format!("fg=<pid 0> hwnd={:?} title={:?}", hwnd.0, title);
        }

        let process = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
            Ok(p) => p,
            Err(e) => {
                return format!(
                    "fg=<OpenProcess failed 0x{:08X}> pid={} hwnd={:?} title={:?}",
                    e.code().0 as u32,
                    pid,
                    hwnd.0,
                    title
                )
            }
        };

        let mut path_buf = [0u16; 1024];
        let path_len = GetModuleFileNameExW(process, None, &mut path_buf);
        let process_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
        let process_name = std::path::Path::new(&process_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let name = if process_name.is_empty() {
            format!("<no exe name, GetLastError={}>", GetLastError().0)
        } else {
            process_name
        };

        format!("fg={} pid={} hwnd={:?} title={:?}", name, pid, hwnd.0, title)
    }
}

fn idle_seconds() -> u64 {
    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut last_input).as_bool() {
            GetTickCount64().saturating_sub(last_input.dwTime as u64) / 1000
        } else {
            u64::MAX // sentinel: the call itself failed
        }
    }
}

/// Mirrors screenshot_service's GDI path up to the raw BGRA buffer.
fn capture_probe() -> String {
    unsafe {
        let fg_hwnd = GetForegroundWindow();
        let hmon = MonitorFromWindow(fg_hwnd, MONITOR_DEFAULTTONEAREST);

        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return "blt=SKIPPED GetMonitorInfoW-failed".to_string();
        }

        let rc = mi.rcMonitor;
        let (mon_x, mon_y) = (rc.left, rc.top);
        let (mon_w, mon_h) = (rc.right - rc.left, rc.bottom - rc.top);

        let desktop = GetDesktopWindow();
        let hdc_screen = GetWindowDC(desktop);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm = CreateCompatibleBitmap(hdc_screen, mon_w, mon_h);
        let hbm_old = SelectObject(hdc_mem, HGDIOBJ(hbm.0));

        let blit = BitBlt(hdc_mem, 0, 0, mon_w, mon_h, hdc_screen, mon_x, mon_y, SRCCOPY);

        let outcome = match blit {
            Err(e) => {
                let code = e.code().0 as u32;
                let label = if code == 0x8007_0006 {
                    "ERROR_INVALID_HANDLE"
                } else {
                    "other"
                };
                format!("blt=FAIL {} 0x{:08X} mon={}x{}", label, code, mon_w, mon_h)
            }
            Ok(()) => {
                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: mon_w,
                        biHeight: -mon_h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [RGBQUAD::default()],
                };

                let mut pixels = vec![0u8; (mon_w * mon_h * 4) as usize];
                let lines = GetDIBits(
                    hdc_mem,
                    hbm,
                    0,
                    mon_h as u32,
                    Some(pixels.as_mut_ptr() as *mut _),
                    &mut bmi,
                    DIB_RGB_COLORS,
                );

                // Sample every 97th pixel — enough to tell black from not-black,
                // cheap enough to run once a second.
                let mut max = 0u8;
                let mut sum: u64 = 0;
                let mut n: u64 = 0;
                let mut zero: u64 = 0;
                for px in pixels.chunks_exact(4).step_by(97) {
                    let v = px[0].max(px[1]).max(px[2]);
                    max = max.max(v);
                    sum += v as u64;
                    n += 1;
                    if v == 0 {
                        zero += 1;
                    }
                }
                let mean = if n > 0 { sum / n } else { 0 };
                let pct_zero = if n > 0 { zero * 100 / n } else { 0 };
                let verdict = if max == 0 { "BLACK" } else { "nonblack" };

                format!(
                    "blt=OK frame={} mean={} max={} zero={}% dibits_lines={} mon={}x{}",
                    verdict, mean, max, pct_zero, lines, mon_w, mon_h
                )
            }
        };

        SelectObject(hdc_mem, hbm_old);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(desktop, hdc_screen);

        outcome
    }
}

fn main() {
    let log_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("lock-probe.log");
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();

    let banner = format!(
        "=== lock-probe started {} ===\nlog file: {}\nLock the machine, wait on the lock screen a few minutes, unlock, then Ctrl-C.\n",
        Local::now().to_rfc3339(),
        log_path.display()
    );
    print!("{}", banner);
    let _ = write!(log, "{}", banner);

    loop {
        let idle = idle_seconds();
        let idle_s = if idle == u64::MAX {
            "idle=GetLastInputInfo-failed".to_string()
        } else {
            format!("idle={}s", idle)
        };

        let line = format!(
            "{}  {}  {}  {}",
            Local::now().format("%H:%M:%S"),
            foreground(),
            capture_probe(),
            idle_s
        );

        println!("{}", line);
        let _ = writeln!(log, "{}", line);
        let _ = log.flush();

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
