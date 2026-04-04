# OCR Pipeline Upgrade — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Screenshots are captured at full resolution, OCR text is extracted via the Windows OCR API, stored in `screenshots.ocr_text`, and available to the classification engine in Plan B.

**Architecture:** Capture full-res JPEG in `spawn_blocking`. Clone it and concurrently: (a) downscale and write to disk (same storage as before), (b) run Windows WinRT OCR on a background async task. After both complete, update the `screenshots` row with `ocr_text`. The full-resolution image is never written to disk. The existing deny-list in `ActivityTracker` already gates which windows get screenshots — OCR is naturally privacy-safe.

**Tech Stack:** Rust, `windows` crate 0.58 WinRT OCR (`Media_Ocr`, `Graphics_Imaging`, `Storage_Streams`), Tauri 2, rusqlite, tokio. `cargo test --features test` for unit testing without a Windows host.

---

### Task 1: Add SQLite migration for `ocr_text` column

**Files:**
- Create: `src-tauri/src/db/migrations/005_add_ocr_text_to_screenshots.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Create the migration SQL file**

```sql
-- Migration 005: Add ocr_text column to screenshots
-- ocr_text is populated asynchronously after capture; initially NULL.
ALTER TABLE screenshots ADD COLUMN ocr_text TEXT;
```

- [ ] **Step 2: Register the migration in `migrations.rs`**

In `src-tauri/src/db/migrations.rs`, add the new entry to the `MIGRATIONS` array after `004_add_device_id_columns`:

```rust
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        include_str!("migrations/001_initial_schema.sql"),
    ),
    (
        "002_add_schema_migrations_table",
        include_str!("migrations/002_add_schema_migrations_table.sql"),
    ),
    (
        "003_sync_queue_additions",
        include_str!("migrations/003_sync_queue_additions.sql"),
    ),
    (
        "004_add_device_id_columns",
        include_str!("migrations/004_add_device_id_columns.sql"),
    ),
    (
        "005_add_ocr_text_to_screenshots",
        include_str!("migrations/005_add_ocr_text_to_screenshots.sql"),
    ),
];
```

- [ ] **Step 3: Build to verify migration compiles**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/db/migrations/005_add_ocr_text_to_screenshots.sql `
        src-tauri/src/db/migrations.rs
git commit -m "feat(db): add ocr_text column to screenshots (migration 005)"
```

---

### Task 2: Add Windows OCR crate features

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add OCR-related Windows features**

In `src-tauri/Cargo.toml`, extend the `[target.'cfg(target_os = "windows")'.dependencies]` block:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_System_SystemInformation",
  "Win32_UI_WindowsAndMessaging",
  "Win32_System_ProcessStatus",
  "Win32_System_Threading",
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Graphics_Imaging",
  "Media_Ocr",
  "Storage_Streams",
] }
```

- [ ] **Step 2: Build to verify the new features resolve**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors (new feature flags pull in the WinRT types).

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/Cargo.toml
git commit -m "feat(deps): add Windows OCR WinRT features to Cargo.toml"
```

---

### Task 3: Create `ocr_service.rs`

**Files:**
- Create: `src-tauri/src/services/ocr_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Write the test double first (TDD)**

Create `src-tauri/src/services/ocr_service.rs` with only the test stub:

```rust
//! OCR text extraction from full-resolution JPEG screenshots.
//! Production path: Windows WinRT OcrEngine (Media_Ocr).
//! Test path: returns a fixed string so screenshot tests run without Windows OCR.

/// Extract text from a full-resolution JPEG byte buffer.
/// Returns `None` if OCR fails or the image contains no readable text.
#[cfg(feature = "test")]
pub async fn extract_text(_jpeg_bytes: &[u8]) -> Option<String> {
    Some("test ocr text".to_string())
}
```

- [ ] **Step 2: Run tests with the test feature to confirm it compiles**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1 | head -30
```

Expected: compiles and existing tests pass.

- [ ] **Step 3: Add the production OCR implementation**

Append to `src-tauri/src/services/ocr_service.rs`:

```rust
#[cfg(not(feature = "test"))]
pub async fn extract_text(jpeg_bytes: &[u8]) -> Option<String> {
    use windows::{
        Graphics::Imaging::BitmapDecoder,
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    };

    // Write JPEG bytes into an in-memory stream
    let stream = InMemoryRandomAccessStream::new().ok()?;
    let writer = DataWriter::CreateDataWriter(&stream).ok()?;
    writer.WriteBytes(jpeg_bytes).ok()?;
    writer.StoreAsync().ok()?.await.ok()?;
    writer.FlushAsync().ok()?.await.ok()?;
    stream.Seek(0).ok()?;

    // Decode JPEG → SoftwareBitmap
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.await.ok()?;
    let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.await.ok()?;

    // Run OCR with the user's profile language
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.await.ok()?;
    let text = result.Text().ok()?.to_string();

    if text.trim().is_empty() { None } else { Some(text) }
}
```

- [ ] **Step 4: Register the module in `services/mod.rs`**

Add `pub mod ocr_service;` to `src-tauri/src/services/mod.rs`:

```rust
pub mod activity_tracker;
pub mod idle_service;
pub mod logger;
pub mod ocr_service;
pub mod screenshot_service;
pub mod sync_service;
pub mod timer_tick;
```

- [ ] **Step 5: Build to verify production path compiles**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: compiles without errors.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/services/ocr_service.rs `
        src-tauri/src/services/mod.rs
git commit -m "feat(ocr): add OcrService with Windows WinRT OCR and test double"
```

---

### Task 4: Refactor `screenshot_service.rs` for full-resolution capture + OCR

**Files:**
- Modify: `src-tauri/src/services/screenshot_service.rs`

The goal: (1) split `capture_screen_jpeg` into `capture_screen_full_res_jpeg` (no resize) + `downscale_jpeg`, (2) update `capture_and_save` to run OCR concurrently with downscale, update `ocr_text` in DB after both complete.

- [ ] **Step 1: Rename and strip the resize from `capture_screen_jpeg` (production path)**

In `src-tauri/src/services/screenshot_service.rs`, rename `capture_screen_jpeg` to `capture_screen_full_res_jpeg` and remove the resize block. The function now returns full-resolution JPEG bytes.

Replace the entire production `capture_screen_jpeg` function (lines 45–168) with:

```rust
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

    unsafe {
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
```

- [ ] **Step 2: Update the test double to match the new name**

Replace the test `capture_screen_jpeg` function with:

```rust
#[cfg(feature = "test")]
fn capture_screen_full_res_jpeg() -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(100, 100, |_, _| Rgb([128u8, 128u8, 128u8]));
    let dyn_img: image::DynamicImage = img.into();
    let mut buf = std::io::Cursor::new(Vec::new());
    dyn_img.write_to(&mut buf, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}
```

- [ ] **Step 3: Add `downscale_jpeg` function**

Add this new function after the capture functions:

```rust
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
```

- [ ] **Step 4: Update `capture_and_save` to use full-res, OCR, then downscale**

Replace the entire `capture_and_save` function:

```rust
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
            let _ = conn.execute(
                "UPDATE screenshots SET ocr_text = ?1 WHERE id = ?2",
                rusqlite::params![text, id],
            );
        }
    } // MutexGuard dropped here

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
```

- [ ] **Step 5: Add unit tests in `screenshot_service.rs`**

Add at the end of `src-tauri/src/services/screenshot_service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscale_jpeg_halves_dimensions() {
        // Create a 100×100 gray JPEG
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

    #[tokio::test]
    async fn ocr_service_returns_text_in_test_mode() {
        let dummy_jpeg: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xD9]; // minimal JPEG header
        let result = crate::services::ocr_service::extract_text(&dummy_jpeg).await;
        assert_eq!(result, Some("test ocr text".to_string()));
    }
}
```

- [ ] **Step 6: Run tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
```

Expected: all tests pass, including `downscale_jpeg_halves_dimensions` and `ocr_service_returns_text_in_test_mode`.

- [ ] **Step 7: Build production binary to verify no regressions**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors or warnings.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/services/screenshot_service.rs
git commit -m "feat(screenshot): full-resolution capture with async OCR and downscale for storage"
```

---

### Task 5: Add `ocr_text` to the `Screenshot` model

**Files:**
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: Add `ocr_text` field to the `Screenshot` struct**

In `src-tauri/src/models/mod.rs`, update the `Screenshot` struct:

```rust
// SQL: screenshots has trigger + device_id + ocr_text (migration 005)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    pub id: String,
    pub file_path: String,
    pub captured_at: String,
    pub window_title: String,
    pub process_name: String,
    pub trigger: String, // "interval" | "window_change"
    pub device_id: String,
    pub ocr_text: Option<String>, // NULL until OCR completes; may remain NULL on OCR failure
}
```

- [ ] **Step 2: Update the `screenshot_list` command to return `ocr_text`**

In `src-tauri/src/commands/screenshot.rs`, update the SELECT query and row mapping to include `ocr_text`. The existing query selects specific columns — add `ocr_text` as the last column:

```rust
// In screenshot_list command, update the SELECT and the row mapping:
// Change the SELECT to:
"SELECT id, file_path, captured_at, window_title, process_name, trigger, device_id, ocr_text \
 FROM screenshots WHERE captured_at BETWEEN ?1 AND ?2 ORDER BY captured_at ASC"

// And update the row closure to:
|row| Ok(Screenshot {
    id:           row.get(0)?,
    file_path:    row.get(1)?,
    captured_at:  row.get(2)?,
    window_title: row.get(3)?,
    process_name: row.get(4)?,
    trigger:      row.get(5)?,
    device_id:    row.get(6)?,
    ocr_text:     row.get(7)?,
})
```

(Read the exact current query in `commands/screenshot.rs` to patch correctly without breaking anything else.)

- [ ] **Step 3: Build and run tests**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
```

Expected: builds and all tests pass.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/models/mod.rs `
        src-tauri/src/commands/screenshot.rs
git commit -m "feat(models): add ocr_text field to Screenshot struct and screenshot_list command"
```
