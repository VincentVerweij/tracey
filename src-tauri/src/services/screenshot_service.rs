use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use chrono::Utc;
use ulid::Ulid;

use crate::commands::AppState;

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

// ─── T043 — Test double (no real GDI) ────────────────────────────────────────

#[cfg(feature = "test")]
fn capture_screen_jpeg() -> Result<Vec<u8>, String> {
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

#[cfg(not(feature = "test"))]
fn capture_screen_jpeg() -> Result<Vec<u8>, String> {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDIBits, GetWindowDC, ReleaseDC, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ, RGBQUAD, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetDesktopWindow, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
    };

    // All Win32 GDI calls are synchronous — must be called from spawn_blocking at call site
    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_screen = GetWindowDC(hwnd);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let hbm = CreateCompatibleBitmap(hdc_screen, screen_width, screen_height);
        // SelectObject requires HGDIOBJ; convert HBITMAP via its inner pointer
        let hbm_old = SelectObject(hdc_mem, HGDIOBJ(hbm.0));

        let blit_result = BitBlt(
            hdc_mem,
            0,
            0,
            screen_width,
            screen_height,
            hdc_screen,
            0,
            0,
            SRCCOPY,
        );

        if blit_result.is_err() {
            // Clean up before returning error
            SelectObject(hdc_mem, hbm_old);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(hwnd, hdc_screen);
            return Err(blit_result.unwrap_err().to_string());
        }

        let bmi_size = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: bmi_size,
                biWidth: screen_width,
                biHeight: -screen_height, // negative = top-down scanlines
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

        let buf_size = (screen_width * screen_height * 4) as usize;
        let mut pixels: Vec<u8> = vec![0u8; buf_size];

        GetDIBits(
            hdc_mem,
            hbm,
            0,
            screen_height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        // Restore and release GDI resources
        SelectObject(hdc_mem, hbm_old);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_screen);

        // Convert device BGRA → RGB
        let w = screen_width as u32;
        let h = screen_height as u32;
        let rgb_pixels: Vec<u8> = pixels
            .chunks_exact(4)
            .flat_map(|bgra| [bgra[2], bgra[1], bgra[0]])
            .collect();

        let img = image::RgbImage::from_raw(w, h, rgb_pixels)
            .ok_or_else(|| "failed to create image from raw pixels".to_string())?;

        // Resize to 50% with Triangle filter (faster than Lanczos3 at half-scale)
        let new_w = w / 2;
        let new_h = h / 2;
        let resized =
            image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle);

        let dyn_resized: image::DynamicImage = resized.into();
        let mut out = std::io::Cursor::new(Vec::new());
        dyn_resized
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .map_err(|e| e.to_string())?;
        Ok(out.into_inner())
    }
}

// ─── Capture + persist ────────────────────────────────────────────────────────

async fn capture_and_save(
    app: &AppHandle,
    trigger: &str,
    window_info: Option<(String, String)>,
) -> Result<(), String> {
    // 1. GDI capture runs in spawn_blocking so it doesn't block the async thread
    let jpeg_bytes = tauri::async_runtime::spawn_blocking(capture_screen_jpeg)
        .await
        .map_err(|e| e.to_string())??;

    // 2. Resolve storage dir and generate filename
    let storage_dir = resolve_storage_dir(app)?;
    let filename = make_screenshot_filename();
    let full_path = storage_dir.join(&filename);

    // 3. Write JPEG to disk
    tokio::fs::write(&full_path, &jpeg_bytes)
        .await
        .map_err(|e| format!("write failed: {}", e))?;

    let path_str = full_path.to_string_lossy().to_string();
    let now = Utc::now().to_rfc3339();
    let id = Ulid::new().to_string().to_lowercase();
    let (process_name, window_title) = window_info.unwrap_or_default();

    // 4. Insert DB row — lock acquired and released before the next await point
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let device_id =
            std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());
        conn.execute(
            "INSERT INTO screenshots \
             (id, file_path, captured_at, window_title, process_name, trigger, device_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, path_str, now, window_title, process_name, trigger, device_id],
        )
        .map_err(|e| e.to_string())?;
    } // MutexGuard dropped here — never held across an await point

    // 5. Emit success event to frontend
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
    let (paths_to_delete, ids_to_delete): (Vec<String>, Vec<String>) = {
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

// ─── T046 — Main service loop ─────────────────────────────────────────────────

pub fn start_screenshot_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_window_key: Option<String> = None;
        let mut last_interval_capture = tokio::time::Instant::now();
        let mut debounce_until: Option<tokio::time::Instant> = None;
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

            // Get current foreground window info — platform access (no DB lock)
            let window_info = {
                let state = app.state::<AppState>();
                // get_foreground_window_info returns Option<WindowInfo>; title field is `title`
                state.platform.get_foreground_window_info().map(|w| {
                    (w.process_name.clone(), w.title.clone())
                })
            };

            let current_key = window_info
                .as_ref()
                .map(|(p, t)| format!("{}|{}", p, t));

            // Window-change detection with 2-second debounce
            if current_key != last_window_key {
                last_window_key = current_key.clone();
                debounce_until = Some(
                    tokio::time::Instant::now() + tokio::time::Duration::from_secs(2),
                );
            }

            let now = tokio::time::Instant::now();
            let interval_elapsed =
                now.duration_since(last_interval_capture).as_secs() >= interval_secs;
            let debounce_fired = debounce_until.map(|d| now >= d).unwrap_or(false);

            if interval_elapsed || debounce_fired {
                let trigger = if debounce_fired { "window_change" } else { "interval" };
                if debounce_fired {
                    debounce_until = None;
                }
                if interval_elapsed { last_interval_capture = now; }

                if let Err(e) = capture_and_save(&app, trigger, window_info).await {
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

            // Hourly cleanup (every 3600 ticks ≈ 1 hour)
            if cleanup_tick % 3600 == 0 {
                cleanup_expired(&app).await;
            }
        }
    });
}
