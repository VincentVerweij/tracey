use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use chrono::Utc;
use ulid::Ulid;

use crate::commands::AppState;
use crate::services::suspension::{Suspension, TrackingLoop};

// ─── T045 — Storage path resolution ──────────────────────────────────────────

fn resolve_storage_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    // Portable mode: screenshots live next to the exe
    let dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or_else(|| "no exe parent".to_string())?
        .join("screenshots");

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
    Ok(canonical)
}

fn make_screenshot_filename() -> String {
    format!("{}.jpg", Ulid::new().to_string().to_lowercase())
}

// Sentinel returned by capture_screen_full_res_jpeg when the session is locked
// (Win32 ERROR_INVALID_HANDLE / 0x80070006). Callers must skip silently.
const ERR_SESSION_LOCKED: &str = "session_locked";

// ─── T043 — Test double (no real GDI) ────────────────────────────────────────

#[cfg(feature = "test")]
fn capture_screen_full_res_jpeg() -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(100, 100, |_, _| Rgb([128u8, 128u8, 128u8]));
    let dyn_img: image::DynamicImage = img.into();
    let mut buf = std::io::Cursor::new(Vec::new());
    dyn_img
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

// ─── T044 — Production GDI capture (Windows) ─────────────────────────────────
/// Captures the active monitor at full resolution. Returns raw JPEG bytes.
/// Must be called from `spawn_blocking` — all Win32 GDI calls are synchronous.
#[cfg(not(feature = "test"))]
fn capture_screen_full_res_jpeg() -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDIBits, GetMonitorInfoW, GetWindowDC, MonitorFromWindow, ReleaseDC, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO,
        MONITOR_DEFAULTTONEAREST, RGBQUAD, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetDesktopWindow, GetForegroundWindow};

    // All Win32 GDI calls are synchronous — must be called from spawn_blocking at call site
    unsafe {
        // Identify which monitor the foreground (active) window sits on.
        let fg_hwnd = GetForegroundWindow();
        let hmon = MonitorFromWindow(fg_hwnd, MONITOR_DEFAULTTONEAREST);

        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if !GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return Err("GetMonitorInfoW failed".to_string());
        }

        let rc = mi.rcMonitor;
        let mon_x = rc.left;
        let mon_y = rc.top;
        let mon_w = rc.right - rc.left;
        let mon_h = rc.bottom - rc.top;

        let desktop = GetDesktopWindow();
        let hdc_screen = GetWindowDC(desktop);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm = CreateCompatibleBitmap(hdc_screen, mon_w, mon_h);
        let hbm_old = SelectObject(hdc_mem, HGDIOBJ(hbm.0));

        let blit_result = BitBlt(hdc_mem, 0, 0, mon_w, mon_h, hdc_screen, mon_x, mon_y, SRCCOPY);
        if let Err(e) = blit_result {
            SelectObject(hdc_mem, hbm_old);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(desktop, hdc_screen);
            // 0x80070006 = ERROR_INVALID_HANDLE: the desktop DC is inaccessible
            // because the session is locked — treat as a silent skip condition.
            if e.code().0 as u32 == 0x80070006 {
                return Err(ERR_SESSION_LOCKED.to_string());
            }
            return Err(e.to_string());
        }

        let bmi_size = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: bmi_size,
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

        let buf_size = (mon_w * mon_h * 4) as usize;
        let mut pixels: Vec<u8> = vec![0u8; buf_size];
        GetDIBits(hdc_mem, hbm, 0, mon_h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);

        SelectObject(hdc_mem, hbm_old);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(desktop, hdc_screen);

        // BGRA → RGB
        let w = mon_w as u32;
        let h = mon_h as u32;
        let rgb: Vec<u8> = pixels.chunks_exact(4).flat_map(|b| [b[2], b[1], b[0]]).collect();
        let img = image::RgbImage::from_raw(w, h, rgb)
            .ok_or_else(|| "failed to build RgbImage".to_string())?;

        // Encode full resolution to JPEG — no resize
        let dyn_img: image::DynamicImage = img.into();
        let mut out = std::io::Cursor::new(Vec::new());
        dyn_img.write_to(&mut out, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }
}

// ─── Downscale ────────────────────────────────────────────────────────────────
/// Downscale a full-resolution JPEG byte buffer to 50% and re-encode as JPEG.
/// Called from `spawn_blocking` because decode+resize is CPU-bound.
fn downscale_jpeg(full_res_jpeg: Vec<u8>) -> Result<Vec<u8>, String> {
    let dyn_img = image::load_from_memory_with_format(&full_res_jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    let w = dyn_img.width() / 2;
    let h = dyn_img.height() / 2;
    let resized = image::imageops::resize(
        &dyn_img.to_rgb8(), w, h, image::imageops::FilterType::Triangle,
    );
    let dyn_resized: image::DynamicImage = resized.into();
    let mut out = std::io::Cursor::new(Vec::new());
    dyn_resized.write_to(&mut out, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

// ─── Capture + persist ────────────────────────────────────────────────────────

async fn capture_and_save(
    app: &AppHandle,
    trigger: &str,
    window_info: Option<(String, String)>,
) -> Result<(), String> {
    // 1. GDI full-resolution capture in spawn_blocking
    let full_res_jpeg = tauri::async_runtime::spawn_blocking(capture_screen_full_res_jpeg)
        .await
        .map_err(|e| e.to_string())??;

    // 2. Clone for OCR (downscale will move the original)
    let ocr_input = full_res_jpeg.clone();

    // 3. Downscale in spawn_blocking — this moves full_res_jpeg
    let small_jpeg = tauri::async_runtime::spawn_blocking(move || downscale_jpeg(full_res_jpeg))
        .await
        .map_err(|e| e.to_string())??;

    // 4. Resolve storage dir and generate filename
    let storage_dir = resolve_storage_dir(app)?;
    let filename = make_screenshot_filename();
    let full_path = storage_dir.join(&filename);

    // 5. Write downscaled JPEG to disk
    tokio::fs::write(&full_path, &small_jpeg)
        .await
        .map_err(|e| format!("write failed: {}", e))?;

    let path_str = full_path.to_string_lossy().to_string();
    let now = Utc::now().to_rfc3339();
    let id = Ulid::new().to_string().to_lowercase();
    let (process_name, window_title) = window_info.unwrap_or_default();

    // 6. Insert DB row (ocr_text = NULL initially) — lock dropped before OCR await
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());
        conn.execute(
            "INSERT INTO screenshots \
             (id, file_path, captured_at, window_title, process_name, trigger, device_id, ocr_text) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            rusqlite::params![id, path_str, now, window_title, process_name, trigger, device_id],
        )
        .map_err(|e| e.to_string())?;
    } // MutexGuard dropped here — never held across an await point

    // 7. Run OCR on full-resolution image (no lock held)
    let ocr_text = crate::services::ocr_service::extract_text(&ocr_input).await;

    // 8. Update ocr_text in DB if extraction succeeded
    if let Some(ref text) = ocr_text {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.db.lock() {
            if let Err(e) = conn.execute(
                "UPDATE screenshots SET ocr_text = ?1 WHERE id = ?2",
                rusqlite::params![text, id],
            ) {
                log::warn!("[screenshot] Failed to persist ocr_text for {id}: {e}");
            }
        }; // semicolon drops lock temporary before state is released
    }

    // 9. Emit success event to frontend
    app.emit(
        "tracey://screenshot-captured",
        serde_json::json!({
            "file_path": path_str,
            "captured_at": now,
            "window_title": window_title,
            "process_name": process_name,
            "trigger": trigger,
        }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ─── T048 — Retention cleanup ─────────────────────────────────────────────────

async fn cleanup_expired(app: &AppHandle) {
    // Phase 1: query what needs deleting — lock released before file I/O awaits
    let (ids_to_delete, paths_to_delete): (Vec<String>, Vec<String>) = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        let retention_days: i64 = conn
            .query_row(
                "SELECT screenshot_retention_days FROM user_preferences LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(30);

        let cutoff = Utc::now() - chrono::Duration::days(retention_days);
        let cutoff_str = cutoff.to_rfc3339();

        let mut stmt = match conn.prepare(
            "SELECT id, file_path FROM screenshots WHERE captured_at < ?1",
        ) {
            Ok(s) => s,
            Err(_) => return,
        };

        let rows: Vec<(String, String)> = match stmt.query_map(
            rusqlite::params![cutoff_str],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ) {
            Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
            Err(_) => return,
        };

        rows.into_iter().unzip()
    }; // MutexGuard dropped — lock released before file deletion awaits

    // Phase 2: delete files from disk
    for path in &paths_to_delete {
        let _ = tokio::fs::remove_file(path).await;
    }

    // Phase 3: delete DB rows — second lock acquisition after all awaits
    if !ids_to_delete.is_empty() {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.db.lock() {
            let placeholders: String = ids_to_delete
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!("DELETE FROM screenshots WHERE id IN ({})", placeholders);
            let params: Vec<&dyn rusqlite::ToSql> = ids_to_delete
                .iter()
                .map(|s| s as &dyn rusqlite::ToSql)
                .collect();
            let _ = conn.execute(&sql, params.as_slice());
        }; // semicolon: Result temporary drops here, before `state` drops at block end
    }
}

// ─── T046 — Capture schedule ──────────────────────────────────────────────────

/// The window-change debounce, reused as the settle after suspension ends.
const DEBOUNCE: tokio::time::Duration = tokio::time::Duration::from_secs(2);

/// Per-tick capture decision: an elapsed interval, or a 2-second-debounced
/// window change. Consults suspension before observing anything.
struct CaptureSchedule {
    last_window_key: Option<String>,
    last_interval_capture: tokio::time::Instant,
    debounce_until: Option<tokio::time::Instant>,
    /// Whether the previous tick was suspended; its falling edge is re-entry.
    suspended_last_tick: bool,
    /// Re-entry armed the settle and its `suspension_end` capture is still due.
    settling: bool,
}

impl CaptureSchedule {
    fn new(now: tokio::time::Instant) -> Self {
        Self {
            last_window_key: None,
            last_interval_capture: now,
            debounce_until: None,
            suspended_last_tick: false,
            settling: false,
        }
    }

    /// Returns the trigger and window info to capture with, or `None` to skip
    /// this tick. A suspended tick skips everything, including `observe` (the
    /// foreground window query) and change-detection updates.
    fn tick(
        &mut self,
        now: tokio::time::Instant,
        interval_secs: u64,
        suspension: &Suspension,
        observe: impl FnOnce() -> Option<(String, String)>,
    ) -> Option<(&'static str, Option<(String, String)>)> {
        if suspension.is_suspended(TrackingLoop::ScreenshotCapture) {
            self.suspended_last_tick = true;
            return None;
        }

        // Re-entry when suspension ends (ADR-0004 decision 5): forget the last
        // window and arm the settle. Re-suspending before it fires is safe: a
        // suspended tick returns above, and the next re-entry arms it again.
        if self.suspended_last_tick {
            self.suspended_last_tick = false;
            self.last_window_key = None;
            self.debounce_until = Some(now + DEBOUNCE);
            self.settling = true;
        }

        let window_info = observe();
        let current_key = window_info
            .as_ref()
            .map(|(p, t)| format!("{}|{}", p, t));

        // Window-change detection with 2-second debounce
        if current_key != self.last_window_key {
            self.last_window_key = current_key;
            self.debounce_until = Some(now + DEBOUNCE);
        }

        let debounce_fired = self.debounce_until.map(|d| now >= d).unwrap_or(false);

        // While settling, only the settled capture may fire. It absorbs any
        // interval that came due during suspension and restarts the interval clock.
        if self.settling {
            if !debounce_fired {
                return None;
            }
            self.settling = false;
            self.debounce_until = None;
            self.last_interval_capture = now;
            return Some(("suspension_end", window_info));
        }

        let interval_elapsed =
            now.duration_since(self.last_interval_capture).as_secs() >= interval_secs;

        if !(interval_elapsed || debounce_fired) {
            return None;
        }
        let trigger = if debounce_fired { "window_change" } else { "interval" };
        if debounce_fired {
            self.debounce_until = None;
        }
        if interval_elapsed {
            self.last_interval_capture = now;
        }
        Some((trigger, window_info))
    }
}

/// The reactive skip: a capture that failed because the desktop is locked drops
/// that frame silently. Defence in depth behind suspension; it never raises or
/// clears `Locked` (ADR-0002).
fn is_reactive_lock_skip(error: &str) -> bool {
    error == ERR_SESSION_LOCKED
}

// ─── T046 — Main service loop ─────────────────────────────────────────────────

pub fn start_screenshot_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut schedule = CaptureSchedule::new(tokio::time::Instant::now());
        let mut cleanup_tick: u64 = 0;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            cleanup_tick += 1;

            // Read prefs — sync lock, released before any await point
            let (interval_secs, _retention_days) = {
                let state = app.state::<AppState>();
                // let x = ...; x pattern: `state` borrow ends at `;`, before block close
                let x = match state.db.lock() {
                    Err(_) => continue,
                    Ok(conn) => {
                        let interval: i64 = conn
                            .query_row(
                                "SELECT screenshot_interval_seconds FROM user_preferences LIMIT 1",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(60);
                        let retention: i64 = conn
                            .query_row(
                                "SELECT screenshot_retention_days FROM user_preferences LIMIT 1",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(30);
                        (interval as u64, retention)
                    }
                }; x
            }; // db lock released

            // Get current foreground window info — platform access (no DB lock),
            // queried only when capture is not suspended
            let state = app.state::<AppState>();
            let decision = schedule.tick(
                tokio::time::Instant::now(),
                interval_secs,
                &state.suspension,
                || {
                    // get_foreground_window_info returns Option<WindowInfo>; title field is `title`
                    state.platform.get_foreground_window_info().map(|w| {
                        (w.process_name.clone(), w.title.clone())
                    })
                },
            );

            if let Some((trigger, window_info)) = decision {
                if let Err(e) = capture_and_save(&app, trigger, window_info).await {
                    if is_reactive_lock_skip(&e) {
                        log::debug!("[screenshot] skipped — session is locked");
                    } else {
                        let _ = app.emit(
                            "tracey://error",
                            serde_json::json!({
                                "component": "screenshot_service",
                                "event": "screenshot_write_failed",
                                "error": e,
                            }),
                        );
                    }
                }
            }

            // Cleanup on startup (tick 1) and then every hour (every 3600 ticks)
            if cleanup_tick == 1 || cleanup_tick % 3600 == 0 {
                cleanup_expired(&app).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, Instant};

    #[test]
    fn downscale_jpeg_halves_dimensions() {
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(100, 100, |_, _| Rgb([200u8, 200u8, 200u8]));
        let dyn_img: image::DynamicImage = img.into();
        let mut buf = std::io::Cursor::new(Vec::new());
        dyn_img.write_to(&mut buf, image::ImageFormat::Jpeg).unwrap();
        let full_res = buf.into_inner();

        let small = downscale_jpeg(full_res).unwrap();

        let decoded = image::load_from_memory_with_format(&small, image::ImageFormat::Jpeg).unwrap();
        assert_eq!(decoded.width(), 50);
        assert_eq!(decoded.height(), 50);
    }


    fn window(title: &str) -> Option<(String, String)> {
        Some(("app.exe".to_string(), title.to_string()))
    }

    /// Ticks once per second from `start` for `secs` ticks, returning each capture trigger.
    fn run_ticks(
        schedule: &mut CaptureSchedule,
        suspension: &Suspension,
        start: Instant,
        secs: std::ops::RangeInclusive<u64>,
        title: &str,
    ) -> Vec<&'static str> {
        secs.filter_map(|s| {
            schedule
                .tick(start + Duration::from_secs(s), 60, suspension, || window(title))
                .map(|(trigger, _)| trigger)
        })
        .collect()
    }

    #[test]
    fn captures_on_interval_when_not_suspended() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        // First observation counts as a window change, so one window_change capture at ~2s.
        let triggers = run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");
        assert_eq!(triggers, vec!["window_change", "interval"]);
    }

    #[test]
    fn no_capture_while_locked() {
        let start = Instant::now();
        let suspension = Suspension::new();
        suspension.raise_locked();
        let mut schedule = CaptureSchedule::new(start);
        let triggers = run_ticks(&mut schedule, &suspension, start, 1..=300, "doc");
        assert!(triggers.is_empty(), "captured while Locked: {triggers:?}");
    }

    #[test]
    fn locked_tick_does_not_query_the_foreground_window() {
        let start = Instant::now();
        let suspension = Suspension::new();
        suspension.raise_locked();
        let mut schedule = CaptureSchedule::new(start);
        let mut queried = false;
        schedule.tick(start + Duration::from_secs(120), 60, &suspension, || {
            queried = true;
            window("doc")
        });
        assert!(!queried);
    }

    #[test]
    fn capture_continues_once_suspension_ends() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        suspension.raise_locked();
        assert!(run_ticks(&mut schedule, &suspension, start, 1..=120, "doc").is_empty());
        suspension.clear_locked();
        let triggers = run_ticks(&mut schedule, &suspension, start, 121..=125, "doc");
        assert!(!triggers.is_empty(), "no capture after Locked cleared");
    }

    #[test]
    fn a_lock_longer_than_the_interval_yields_one_suspension_end_capture() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");

        suspension.raise_locked();
        assert!(run_ticks(&mut schedule, &suspension, start, 61..=300, "doc").is_empty());
        suspension.clear_locked();

        // Same window as before the lock, interval long overdue: one settled capture only.
        let triggers = run_ticks(&mut schedule, &suspension, start, 301..=310, "doc");
        assert_eq!(triggers, vec!["suspension_end"]);
    }

    #[test]
    fn the_interval_clock_restarts_from_the_suspension_end_capture() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");
        suspension.raise_locked();
        run_ticks(&mut schedule, &suspension, start, 61..=300, "doc");
        suspension.clear_locked();

        // Re-entry at 301 settles at 303, so the next interval capture is due at 363.
        let captures: Vec<(u64, &str)> = (301..=400)
            .filter_map(|s| {
                schedule
                    .tick(start + Duration::from_secs(s), 60, &suspension, || window("doc"))
                    .map(|(trigger, _)| (s, trigger))
            })
            .collect();
        assert_eq!(captures, vec![(303, "suspension_end"), (363, "interval")]);
    }

    #[test]
    fn re_locking_during_the_settle_defers_the_capture_to_the_next_unlock() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");
        suspension.raise_locked();
        run_ticks(&mut schedule, &suspension, start, 61..=300, "doc");
        suspension.clear_locked();

        // Unlock arms the settle, then the session locks again before it fires.
        assert!(run_ticks(&mut schedule, &suspension, start, 301..=301, "doc").is_empty());
        suspension.raise_locked();
        let while_locked = run_ticks(&mut schedule, &suspension, start, 302..=400, "doc");
        assert!(while_locked.is_empty(), "captured while Locked: {while_locked:?}");

        suspension.clear_locked();
        let triggers = run_ticks(&mut schedule, &suspension, start, 401..=410, "doc");
        assert_eq!(triggers, vec!["suspension_end"]);
    }

    #[test]
    fn a_window_switch_during_the_settle_still_yields_one_suspension_end_capture() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");
        suspension.raise_locked();
        run_ticks(&mut schedule, &suspension, start, 61..=300, "doc");
        suspension.clear_locked();

        let mut triggers = run_ticks(&mut schedule, &suspension, start, 301..=302, "doc");
        triggers.extend(run_ticks(&mut schedule, &suspension, start, 303..=310, "mail"));
        assert_eq!(triggers, vec!["suspension_end"]);
    }

    #[test]
    fn clearing_locked_while_another_reason_holds_is_not_a_suspension_end() {
        let start = Instant::now();
        let suspension = Suspension::new();
        let mut schedule = CaptureSchedule::new(start);
        run_ticks(&mut schedule, &suspension, start, 1..=60, "doc");
        suspension.raise_locked();
        suspension.raise_other_reason();
        run_ticks(&mut schedule, &suspension, start, 61..=300, "doc");
        suspension.clear_locked();

        let triggers = run_ticks(&mut schedule, &suspension, start, 301..=400, "doc");
        assert!(triggers.is_empty(), "captured while still suspended: {triggers:?}");
    }

    #[test]
    fn locked_desktop_capture_failure_is_skipped_silently() {
        assert!(is_reactive_lock_skip(ERR_SESSION_LOCKED));
        assert!(!is_reactive_lock_skip("GetMonitorInfoW failed"));
    }

    #[cfg(feature = "test")]
    #[tokio::test]
    async fn ocr_service_returns_text_in_test_mode() {
        let dummy_jpeg: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xD9];
        let result = crate::services::ocr_service::extract_text(&dummy_jpeg).await;
        assert_eq!(result, Some("test ocr text".to_string()));
    }
}
