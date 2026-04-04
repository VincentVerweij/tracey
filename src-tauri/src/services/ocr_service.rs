//! OCR text extraction from full-resolution JPEG screenshots.
//! Production path: Windows WinRT OcrEngine (Media_Ocr).
//! Test path: returns a fixed string so screenshot tests run without Windows OCR.

/// Extract text from a full-resolution JPEG byte buffer.
/// Returns `None` if OCR fails or the image contains no readable text.
#[cfg(feature = "test")]
pub async fn extract_text(_jpeg_bytes: &[u8]) -> Option<String> {
    Some("test ocr text".to_string())
}

#[cfg(not(feature = "test"))]
pub async fn extract_text(jpeg_bytes: &[u8]) -> Option<String> {
    use windows::{
        Graphics::Imaging::BitmapDecoder,
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    };

    // Write JPEG bytes into an in-memory stream — all fallible steps return Option via ?
    let stream = InMemoryRandomAccessStream::new().ok()?;
    let writer = DataWriter::CreateDataWriter(&stream).ok()?;
    writer.WriteBytes(jpeg_bytes).ok()?;
    writer.StoreAsync().ok()?.get().ok()?;
    writer.FlushAsync().ok()?.get().ok()?;
    stream.Seek(0).ok()?;

    // Decode JPEG → SoftwareBitmap
    let decoder = BitmapDecoder::CreateAsync(&stream).ok()?.get().ok()?;
    let bitmap = decoder.GetSoftwareBitmapAsync().ok()?.get().ok()?;

    // Run OCR with the user's profile language
    let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok()?;
    let result = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;
    let text = result.Text().ok()?.to_string();

    if text.trim().is_empty() { None } else { Some(text) }
}
