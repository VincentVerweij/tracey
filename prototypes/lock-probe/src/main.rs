//! PROTOTYPE — throwaway instrumentation for wayfinder tickets #66 and #73 (map #65).
//!
//! #66: once a second it records, on one `tick` line:
//!   - the foreground window (process name + title), exactly as
//!     `PlatformHooks::get_foreground_window_info()` sees it, or why it saw nothing
//!   - the `BitBlt` outcome: OK, ERROR_INVALID_HANDLE (0x80070006), or another HRESULT
//!   - whether the captured frame is actually non-black (mean / max / % zero pixels)
//!   - `get_idle_seconds()`, mirroring the production idle query
//!
//! #73 adds, on the same timeline:
//!   - `wts=` on every tick: `WTSQuerySessionInformationW(…, WTSSessionInfoEx, …)`
//!     → `WTSINFOEX_LEVEL1.SessionFlags` (LOCK / UNLOCK / UNKNOWN), plus the
//!     connect state (Active / Connected / Disconnected …) and session id
//!   - an `EVENT` line for every `WM_WTSSESSION_CHANGE`, timestamped to the ms the
//!     moment it is dispatched, with `SessionFlags` re-queried inside the handler
//!
//! Events are registered (`NOTIFY_FOR_THIS_SESSION`) on TWO windows, so the log
//! also shows whether both kinds receive them:
//!   - `msgonly` — a message-only window (`HWND_MESSAGE` parent)
//!   - `toplevel` — a hidden, never-shown top-level window, the closest thing to
//!     the real tray-hidden Tauri window that production will subclass
//!
//! The main thread only pumps messages; ticks run on their own thread so a slow
//! `BitBlt` around a desktop switch cannot delay event timestamps.
//!
//! Capture mirrors `screenshot_service::capture_screen_full_res_jpeg()` up to the
//! pixel buffer, then stops — nothing is encoded or written as an image.
//!
//! Every line also lands in `lock-probe.log` next to the exe, because the console
//! is not readable while the machine is locked.
//!
//! No error handling, no abstractions, no tests. It answers one question.

use chrono::Local;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
    GetMonitorInfoW, GetWindowDC, MonitorFromWindow, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    RGBQUAD, SRCCOPY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSRegisterSessionNotification, WTSSessionInfoEx,
    NOTIFY_FOR_THIS_SESSION, WTSINFOEXW, WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION,
    WTS_SESSIONSTATE_LOCK, WTS_SESSIONSTATE_UNKNOWN, WTS_SESSIONSTATE_UNLOCK,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetDesktopWindow, GetForegroundWindow,
    GetMessageW, GetWindowTextW, GetWindowThreadProcessId, RegisterClassW, TranslateMessage,
    HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

static LOG: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

fn emit(line: &str) {
    println!("{}", line);
    let mut f = LOG.get().unwrap().lock().unwrap();
    let _ = writeln!(f, "{}", line);
    let _ = f.flush();
}

fn now() -> String {
    Local::now().format("%H:%M:%S%.3f").to_string()
}

/// `WTSINFOEX_LEVEL1.SessionFlags` + connect state + session id, as one token.
fn wts_state() -> String {
    unsafe {
        let mut buf = PWSTR::null();
        let mut bytes = 0u32;
        if let Err(e) = WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buf,
            &mut bytes,
        ) {
            return format!("wts=QUERY-FAILED 0x{:08X}", e.code().0 as u32);
        }
        let info = &*(buf.0 as *const WTSINFOEXW);
        let out = if info.Level != 1 {
            format!("wts=<level {}>", info.Level)
        } else {
            let l1 = info.Data.WTSInfoExLevel1;
            let flags = match l1.SessionFlags as u32 {
                WTS_SESSIONSTATE_LOCK => "LOCK".to_string(),
                WTS_SESSIONSTATE_UNLOCK => "UNLOCK".to_string(),
                WTS_SESSIONSTATE_UNKNOWN => "UNKNOWN".to_string(),
                other => format!("<raw {}>", other),
            };
            let conn = match l1.SessionState.0 {
                0 => "Active",
                1 => "Connected",
                2 => "ConnectQuery",
                3 => "Shadow",
                4 => "Disconnected",
                5 => "Idle",
                6 => "Listen",
                7 => "Reset",
                8 => "Down",
                9 => "Init",
                _ => "?",
            };
            format!("wts={} conn={} sid={}", flags, conn, l1.SessionId)
        };
        WTSFreeMemory(buf.0 as *mut _);
        out
    }
}

fn wts_code(code: usize) -> &'static str {
    match code {
        0x1 => "WTS_CONSOLE_CONNECT",
        0x2 => "WTS_CONSOLE_DISCONNECT",
        0x3 => "WTS_REMOTE_CONNECT",
        0x4 => "WTS_REMOTE_DISCONNECT",
        0x5 => "WTS_SESSION_LOGON",
        0x6 => "WTS_SESSION_LOGOFF",
        0x7 => "WTS_SESSION_LOCK",
        0x8 => "WTS_SESSION_UNLOCK",
        0x9 => "WTS_SESSION_REMOTE_CONTROL",
        0xA => "WTS_SESSION_CREATE",
        0xB => "WTS_SESSION_TERMINATE",
        _ => "<unknown code>",
    }
}

static MSGONLY: OnceLock<isize> = OnceLock::new();

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE {
        let which = if MSGONLY.get() == Some(&(hwnd.0 as isize)) {
            "msgonly"
        } else {
            "toplevel"
        };
        emit(&format!(
            "{}  EVENT {} (0x{:X}) on={} event_sid={}  {}",
            now(),
            wts_code(wparam.0),
            wparam.0,
            which,
            lparam.0,
            wts_state()
        ));
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn make_window(class: PCWSTR, parent: Option<HWND>, label: &str) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("lock-probe"),
            WS_OVERLAPPEDWINDOW, // never shown
            0,
            0,
            0,
            0,
            parent.unwrap_or_default(),
            None,
            GetModuleHandleW(None).unwrap(),
            None,
        )
        .unwrap();
        let reg = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        emit(&format!(
            "{}  register {} hwnd={:?} -> {}",
            now(),
            label,
            hwnd.0,
            match reg {
                Ok(()) => "OK".to_string(),
                Err(e) => format!("FAILED 0x{:08X}", e.code().0 as u32),
            }
        ));
        hwnd
    }
}

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
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap();
    LOG.set(Mutex::new(log)).unwrap();

    emit(&format!(
        "=== lock-probe started {} ===\nlog file: {}\nRun each scenario from RUNBOOK-73.md, then Ctrl-C.",
        Local::now().to_rfc3339(),
        log_path.display()
    ));
    emit(&format!("{}  startup-seed  {}", now(), wts_state()));

    unsafe {
        let class = w!("lock-probe-wts");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: GetModuleHandleW(None).unwrap().into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let msgonly = make_window(class, Some(HWND_MESSAGE), "msgonly");
        MSGONLY.set(msgonly.0 as isize).unwrap();
        let _toplevel = make_window(class, None, "toplevel");
    }

    // Type a line + Enter at any time to drop a scenario marker into the timeline.
    std::thread::spawn(|| {
        for line in std::io::stdin().lines() {
            emit(&format!("{}  MARK {}", now(), line.unwrap()));
        }
    });

    std::thread::spawn(|| loop {
        let idle = idle_seconds();
        let idle_s = if idle == u64::MAX {
            "idle=GetLastInputInfo-failed".to_string()
        } else {
            format!("idle={}s", idle)
        };
        // Query WTS first: it is the value production would reconcile against
        // right before deciding to capture.
        let wts = wts_state();
        emit(&format!(
            "{}  tick  {}  {}  {}  {}",
            now(),
            wts,
            foreground(),
            capture_probe(),
            idle_s
        ));
        std::thread::sleep(std::time::Duration::from_secs(1));
    });

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
