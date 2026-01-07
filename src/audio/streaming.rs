//! Streaming buffer and download utilities for audio playback
//!
//! This module provides:
//! - `SharedBuffer`: Thread-safe memory buffer for streaming audio
//! - `StreamingBuffer`: Read+Seek wrapper for rodio's Decoder
//! - `StreamingEvent`: Download progress events
//! - `start_buffer_download()`: Unified download function
//!
//! Download thread writes to buffer, playback thread reads from it.
//! Blocks when data is not yet available.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use futures_util::StreamExt;
use parking_lot::{Condvar, Mutex, RwLock};

// ============ Constants ============

/// When remaining buffered data falls below this, enter Buffering state.
pub const LOW_WATER_MARK_BYTES: u64 = 40 * 1024;

/// When buffered data exceeds this, exit Buffering and resume Playing.
pub const HIGH_WATER_MARK_BYTES: u64 = 400 * 1024;

/// Valid audio extensions for URL parsing
const VALID_AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "aac", "ogg", "wav"];

// ============ Format Detection Helpers ============

/// Extract audio file extension from URL path
///
/// # Example
/// ```
/// let ext = extract_extension_from_url("http://example.com/song.flac?token=xxx");
/// assert_eq!(ext, Some("flac".to_string()));
/// ```
pub fn extract_extension_from_url(url: &str) -> Option<String> {
    // Parse URL and get path
    let url_parsed = reqwest::Url::parse(url).ok()?;
    let path = url_parsed.path();

    // Get the last segment (filename)
    let filename = path.rsplit('/').next()?;

    // Extract extension
    let ext = filename.rsplit('.').next()?.to_lowercase();

    // Validate it's a known audio extension
    if VALID_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ext)
    } else {
        None
    }
}

/// Map Content-Type header to file extension
///
/// # Example
/// ```
/// let ext = content_type_to_extension("audio/flac");
/// assert_eq!(ext, Some("flac".to_string()));
/// ```
pub fn content_type_to_extension(content_type: &str) -> Option<String> {
    // Extract MIME type (ignore parameters like charset)
    let mime = content_type.split(';').next()?.trim().to_lowercase();

    match mime.as_str() {
        "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
        "audio/flac" | "audio/x-flac" => Some("flac".to_string()),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" | "audio/aac" => Some("m4a".to_string()),
        "audio/ogg" | "audio/vorbis" => Some("ogg".to_string()),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav".to_string()),
        _ => None,
    }
}

// ============ Events ============

/// Events from the streaming downloader
#[derive(Debug, Clone)]
pub enum StreamingEvent {
    /// Enough data downloaded, playback can start
    Playable,
    /// Download progress update (downloaded_bytes, total_bytes)
    Progress(u64, u64),
    /// Download complete
    Complete,
    /// Download error
    Error(String),
}

#[derive(Debug, Clone)]
pub enum BufferEvent {
    DataAppended { downloaded: u64, total: u64 },
    Complete,
}

/// Estimate content size from duration (40KB/s at 320kbps)
pub fn estimate_size_from_duration(duration_secs: u64) -> u64 {
    let estimated = duration_secs * 40 * 1024;
    if estimated > 0 {
        estimated
    } else {
        10 * 1024 * 1024
    } // 10MB default
}

// ============ Buffer State ============

/// Inner shared state
struct SharedBufferInner {
    /// Audio data bytes
    data: RwLock<Vec<u8>>,
    /// Total file size (from Content-Length)
    total_size: AtomicU64,
    /// Bytes downloaded so far
    downloaded: AtomicU64,
    /// Download complete flag
    complete: AtomicBool,
    /// Cancelled flag
    cancelled: AtomicBool,
    /// Error message if any
    error: RwLock<Option<String>>,
    /// Condition variable for waiting readers
    data_available: Condvar,
    /// Mutex for condvar
    wait_mutex: Mutex<()>,
    /// Buffer event callback
    buffer_callback: RwLock<Option<Box<dyn Fn(BufferEvent) + Send + Sync>>>,
}

/// Thread-safe shared buffer for streaming audio
///
/// Download thread calls `append()` to add data.
/// Playback thread uses `StreamingBuffer` which calls `read_at()`.
#[derive(Clone)]
pub struct SharedBuffer {
    inner: Arc<SharedBufferInner>,
}

impl std::fmt::Debug for SharedBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedBuffer")
            .field("total_size", &self.total_size())
            .field("downloaded", &self.downloaded())
            .field("complete", &self.is_complete())
            .finish()
    }
}

impl SharedBuffer {
    /// Create a new shared buffer with expected total size
    pub fn new(total_size: u64) -> Self {
        Self {
            inner: Arc::new(SharedBufferInner {
                data: RwLock::new(Vec::with_capacity(total_size as usize)),
                total_size: AtomicU64::new(total_size),
                downloaded: AtomicU64::new(0),
                complete: AtomicBool::new(false),
                cancelled: AtomicBool::new(false),
                error: RwLock::new(None),
                data_available: Condvar::new(),
                wait_mutex: Mutex::new(()),
                buffer_callback: RwLock::new(None),
            }),
        }
    }

    pub fn set_buffer_callback<F>(&self, callback: F)
    where
        F: Fn(BufferEvent) + Send + Sync + 'static,
    {
        *self.inner.buffer_callback.write() = Some(Box::new(callback));
    }

    pub fn clear_buffer_callback(&self) {
        *self.inner.buffer_callback.write() = None;
    }

    fn notify_callback(&self, event: BufferEvent) {
        if let Some(callback) = self.inner.buffer_callback.read().as_ref() {
            callback(event);
        }
    }

    /// Append data from download thread
    pub fn append(&self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }

        {
            let mut data = self.inner.data.write();
            data.extend_from_slice(chunk);
        }

        self.inner
            .downloaded
            .fetch_add(chunk.len() as u64, Ordering::Release);

        // Notify waiting readers
        self.inner.data_available.notify_all();

        // Notify callback of data appended
        let downloaded = self.inner.downloaded.load(Ordering::Acquire);
        let total = self.inner.total_size.load(Ordering::Acquire);
        self.notify_callback(BufferEvent::DataAppended { downloaded, total });
    }

    /// Read data at position, blocking if not available
    ///
    /// Returns number of bytes read, or error if cancelled/failed.
    /// Blocks and waits for data when reading positions not yet downloaded.
    pub fn read_at(&self, position: u64, buf: &mut [u8]) -> io::Result<usize> {
        // Check for cancellation/error first
        if self.inner.cancelled.load(Ordering::Acquire) {
            tracing::debug!("read_at: cancelled at position {}", position);
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Download cancelled",
            ));
        }

        if let Some(err) = self.inner.error.read().as_ref() {
            tracing::debug!("read_at: error at position {}: {}", position, err);
            return Err(io::Error::new(io::ErrorKind::Other, err.clone()));
        }

        // Wait for data if needed
        let mut wait_count = 0;
        loop {
            let downloaded = self.inner.downloaded.load(Ordering::Acquire);
            let total = self.inner.total_size.load(Ordering::Acquire);
            let is_complete = self.inner.complete.load(Ordering::Acquire);

            // Only return EOF if download is complete AND position is beyond actual downloaded data
            if is_complete && position >= downloaded {
                tracing::debug!(
                    "read_at: EOF at position {} (downloaded: {}, complete: true)",
                    position,
                    downloaded
                );
                return Ok(0);
            }

            if position < downloaded {
                // Data available, read it
                let data = self.inner.data.read();
                let available = downloaded.saturating_sub(position) as usize;
                let to_read = buf.len().min(available);

                if to_read > 0 {
                    let start = position as usize;
                    buf[..to_read].copy_from_slice(&data[start..start + to_read]);
                    if wait_count > 0 {
                        tracing::debug!(
                            "read_at: resumed at position {} after {} waits",
                            position,
                            wait_count
                        );
                    }
                    return Ok(to_read);
                }
            }

            // Check cancellation again before waiting
            if self.inner.cancelled.load(Ordering::Acquire) {
                tracing::debug!("read_at: cancelled while waiting at position {}", position);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Download cancelled",
                ));
            }

            if let Some(err) = self.inner.error.read().as_ref() {
                tracing::debug!(
                    "read_at: error while waiting at position {}: {}",
                    position,
                    err
                );
                return Err(io::Error::new(io::ErrorKind::Other, err.clone()));
            }

            // Data not yet available, wait for download to progress
            // This enables seeking to positions that haven't been downloaded yet
            wait_count += 1;

            if wait_count == 1 {
                tracing::info!(
                    "read_at: blocking at position {} (downloaded: {}/{}, complete: {})",
                    position,
                    downloaded,
                    total,
                    is_complete
                );
            } else if wait_count % 10 == 0 {
                tracing::info!(
                    "read_at: still waiting for data at position {} (downloaded: {}/{}, complete: {}, wait #{})",
                    position,
                    downloaded,
                    total,
                    is_complete,
                    wait_count
                );
            }
            let mut guard = self.inner.wait_mutex.lock();
            let _ = self
                .inner
                .data_available
                .wait_for(&mut guard, std::time::Duration::from_millis(100));
        }
    }

    /// Get downloaded bytes count
    pub fn downloaded(&self) -> u64 {
        self.inner.downloaded.load(Ordering::Acquire)
    }

    /// Get total size
    pub fn total_size(&self) -> u64 {
        self.inner.total_size.load(Ordering::Acquire)
    }

    /// Update total size (when actual content-length is received)
    pub fn set_total_size(&self, size: u64) {
        self.inner.total_size.store(size, Ordering::Release);
    }

    /// Check if download is complete
    pub fn is_complete(&self) -> bool {
        self.inner.complete.load(Ordering::Acquire)
    }

    /// Check if cancelled
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    /// Cancel the download
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
        self.inner.data_available.notify_all();
    }

    /// Set error state
    pub fn set_error(&self, error: String) {
        *self.inner.error.write() = Some(error);
        self.inner.data_available.notify_all();
    }

    /// Mark download as complete
    pub fn mark_complete(&self) {
        self.inner.complete.store(true, Ordering::Release);
        self.inner.data_available.notify_all();
        self.notify_callback(BufferEvent::Complete);
    }

    /// Get download progress as fraction (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        let total = self.inner.total_size.load(Ordering::Acquire);
        if total == 0 {
            return 0.0;
        }
        let downloaded = self.inner.downloaded.load(Ordering::Acquire);
        (downloaded as f32 / total as f32).min(1.0)
    }
}

/// Streaming buffer that implements Read + Seek for rodio Decoder
///
/// Wraps a SharedBuffer and maintains a read position.
/// Blocks on read() when data is not yet available.
pub struct StreamingBuffer {
    shared: SharedBuffer,
    position: u64,
}

impl StreamingBuffer {
    /// Create a new streaming buffer
    pub fn new(shared: SharedBuffer) -> Self {
        Self {
            shared,
            position: 0,
        }
    }

    /// Get reference to the shared buffer (for checking state)
    pub fn shared(&self) -> &SharedBuffer {
        &self.shared
    }
}

impl Read for StreamingBuffer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.shared.read_at(self.position, buf)?;
        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl Seek for StreamingBuffer {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total = self.shared.total_size();
        let downloaded = self.shared.downloaded();
        let is_complete = self.shared.is_complete();

        tracing::debug!(
            "StreamingBuffer::seek({:?}) - total: {}, downloaded: {}, complete: {}, current_pos: {}",
            pos,
            total,
            downloaded,
            is_complete,
            self.position
        );

        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => {
                // For SeekFrom::End, we need the actual file size
                // If download is complete, use downloaded size (most accurate)
                // Otherwise use total_size (which might be estimated)
                let size = if is_complete {
                    downloaded
                } else if total > 0 {
                    total
                } else {
                    tracing::warn!("SeekFrom::End failed: unknown file size");
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cannot seek from end: unknown file size",
                    ));
                };
                tracing::debug!("SeekFrom::End({}) using size {}", offset, size);
                size as i64 + offset
            }
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_pos < 0 {
            tracing::warn!("Seek to negative position: {}", new_pos);
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek to negative position",
            ));
        }

        // Allow seeking to any position - read_at() will block and wait for data
        // This enables seeking to positions that haven't been downloaded yet
        self.position = new_pos as u64;

        tracing::debug!(
            "StreamingBuffer seek to {} (downloaded: {}, total: {}, complete: {})",
            self.position,
            downloaded,
            total,
            is_complete
        );

        Ok(self.position)
    }
}

// ============ Unified Download Function ============

/// Start downloading audio to a SharedBuffer
///
/// This is the unified download function used by both song_resolver and preload_manager.
/// It spawns a background task that:
/// 1. Downloads from the URL (gets content_length from response)
/// 2. Writes to SharedBuffer (for playback)
/// 3. Writes to temp file during download, then renames with correct extension on completion
/// 4. Sends events via the provided channel
///
/// Returns immediately with the SharedBuffer. The download continues in background.
/// Content length is obtained from the GET response, not from a separate HEAD request.
///
/// # Atomic Write Strategy
/// - During download: writes to `{cache_path}.tmp`
/// - On completion: detects format from magic bytes, renames to `{stem}.{ext}`
/// - On failure/cancel: deletes temp file
pub fn start_buffer_download(
    url: String,
    cache_path: PathBuf,
    event_tx: Option<tokio::sync::mpsc::Sender<StreamingEvent>>,
) -> SharedBuffer {
    // Create buffer with 0 initial size - will be updated when we get content_length from response
    let shared_buffer = SharedBuffer::new(0);
    let buffer_clone = shared_buffer.clone();

    // Pre-extract extension hints from URL for fallback
    let url_extension = extract_extension_from_url(&url);

    tokio::spawn(async move {
        let http_client = reqwest::Client::new();
        let response = match http_client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                buffer_clone.set_error(format!("HTTP {}", status));
                if let Some(tx) = &event_tx {
                    let _ = tx
                        .send(StreamingEvent::Error(format!("HTTP {}", status)))
                        .await;
                }
                return;
            }
            Err(e) => {
                buffer_clone.set_error(e.to_string());
                if let Some(tx) = &event_tx {
                    let _ = tx.send(StreamingEvent::Error(e.to_string())).await;
                }
                return;
            }
        };

        // Get content-length from response - this is the accurate file size
        let content_length = response.content_length().unwrap_or(0);
        if content_length > 0 {
            buffer_clone.set_total_size(content_length);
            tracing::debug!("Got content_length from response: {} bytes", content_length);
        } else {
            tracing::warn!("Response has no content_length header");
        }

        // Get Content-Type for format detection fallback
        let content_type_extension = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(content_type_to_extension);

        // Create temp file for atomic write
        let temp_path = cache_path.with_extension("tmp");
        let mut file = match std::fs::File::create(&temp_path) {
            Ok(f) => Some(f),
            Err(e) => {
                tracing::warn!("Could not create temp cache file {:?}: {}", temp_path, e);
                None
            }
        };

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut playable_sent = false;
        let total_size = buffer_clone.total_size();

        while let Some(chunk_result) = stream.next().await {
            if buffer_clone.is_cancelled() {
                tracing::debug!("Buffer download cancelled, cleaning up temp file");
                // Clean up temp file on cancel
                if file.is_some() {
                    drop(file);
                    let _ = std::fs::remove_file(&temp_path);
                }
                return;
            }

            match chunk_result {
                Ok(chunk) => {
                    let chunk_len = chunk.len() as u64;
                    buffer_clone.append(&chunk);

                    if let Some(ref mut f) = file {
                        use std::io::Write;
                        if let Err(e) = f.write_all(&chunk) {
                            tracing::warn!("Cache file write error: {}", e);
                            file = None;
                            // Clean up failed temp file
                            let _ = std::fs::remove_file(&temp_path);
                        }
                    }

                    downloaded += chunk_len;

                    if !playable_sent && downloaded >= HIGH_WATER_MARK_BYTES {
                        if let Some(tx) = &event_tx {
                            let _ = tx.send(StreamingEvent::Playable).await;
                        }
                        playable_sent = true;
                        tracing::debug!("Buffer playable after {} bytes", downloaded);
                    }

                    if let Some(tx) = &event_tx {
                        let _ = tx
                            .send(StreamingEvent::Progress(downloaded, total_size))
                            .await;
                    }
                }
                Err(e) => {
                    let error_msg = format!("Stream error: {}", e);
                    buffer_clone.set_error(error_msg.clone());
                    if let Some(tx) = &event_tx {
                        let _ = tx.send(StreamingEvent::Error(error_msg)).await;
                    }
                    // Clean up temp file on error
                    if file.is_some() {
                        drop(file);
                        let _ = std::fs::remove_file(&temp_path);
                    }
                    return;
                }
            }
        }

        // Download complete - finalize cache file with correct extension
        if let Some(ref mut f) = file {
            use std::io::Write;
            let _ = f.flush();
            drop(file);

            // Detect format from downloaded data (magic bytes)
            let detected_ext = if let Ok(bytes) = std::fs::read(&temp_path) {
                crate::utils::detect_audio_format(&bytes)
            } else {
                "mp3"
            };

            // Determine final extension: magic bytes > URL > Content-Type > default
            let final_ext = if detected_ext != "mp3" {
                detected_ext.to_string()
            } else if let Some(ref ext) = url_extension {
                ext.clone()
            } else if let Some(ref ext) = content_type_extension {
                ext.clone()
            } else {
                "mp3".to_string()
            };

            // Get stem from original cache_path and create final path with correct extension
            let stem = cache_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let parent = cache_path.parent().unwrap_or(std::path::Path::new("."));
            let final_path = parent.join(format!("{}.{}", stem, final_ext));

            // Rename temp file to final path
            if let Err(e) = std::fs::rename(&temp_path, &final_path) {
                tracing::warn!("Failed to rename temp file to {:?}: {}", final_path, e);
                // Try to clean up temp file
                let _ = std::fs::remove_file(&temp_path);
            } else {
                tracing::info!(
                    "Cache file saved: {:?} (detected: {}, url: {:?}, content-type: {:?})",
                    final_path,
                    detected_ext,
                    url_extension,
                    content_type_extension
                );
            }
        }

        buffer_clone.mark_complete();
        if let Some(tx) = &event_tx {
            let _ = tx.send(StreamingEvent::Complete).await;
        }
        tracing::debug!("Buffer download complete: {} bytes", downloaded);

        if !playable_sent {
            if let Some(tx) = &event_tx {
                let _ = tx.send(StreamingEvent::Playable).await;
            }
        }
    });

    shared_buffer
}

/// Wait for buffer to become playable (with timeout)
pub async fn wait_for_playable(
    event_rx: &mut tokio::sync::mpsc::Receiver<StreamingEvent>,
    timeout_secs: u64,
) -> bool {
    let timeout = tokio::time::Duration::from_secs(timeout_secs);
    tokio::time::timeout(timeout, async {
        while let Some(event) = event_rx.recv().await {
            match event {
                StreamingEvent::Playable | StreamingEvent::Complete => return true,
                StreamingEvent::Error(e) => {
                    tracing::error!("Download error: {}", e);
                    return false;
                }
                StreamingEvent::Progress(_, _) => continue,
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_buffer_new() {
        let buffer = SharedBuffer::new(1000);
        assert_eq!(buffer.total_size(), 1000);
        assert_eq!(buffer.downloaded(), 0);
        assert!(!buffer.is_complete());
        assert!(!buffer.is_cancelled());
    }

    #[test]
    fn test_shared_buffer_append() {
        let buffer = SharedBuffer::new(100);

        buffer.append(&[1, 2, 3, 4, 5]);
        assert_eq!(buffer.downloaded(), 5);

        buffer.append(&[6, 7, 8, 9, 10]);
        assert_eq!(buffer.downloaded(), 10);
    }

    #[test]
    fn test_shared_buffer_read_at_available_data() {
        let buffer = SharedBuffer::new(100);
        buffer.append(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let mut buf = [0u8; 5];
        let bytes_read = buffer.read_at(0, &mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(buf, [1, 2, 3, 4, 5]);

        let bytes_read = buffer.read_at(5, &mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(buf, [6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_shared_buffer_read_at_partial() {
        let buffer = SharedBuffer::new(100);
        buffer.append(&[1, 2, 3]);

        // Request more than available
        let mut buf = [0u8; 10];
        let bytes_read = buffer.read_at(0, &mut buf).unwrap();
        assert_eq!(bytes_read, 3);
        assert_eq!(&buf[..3], &[1, 2, 3]);
    }

    #[test]
    fn test_shared_buffer_read_at_eof_when_complete() {
        let buffer = SharedBuffer::new(10);
        buffer.append(&[1, 2, 3, 4, 5]);
        buffer.mark_complete();

        // Reading at position beyond downloaded data should return EOF
        let mut buf = [0u8; 5];
        let bytes_read = buffer.read_at(5, &mut buf).unwrap();
        assert_eq!(bytes_read, 0); // EOF
    }

    #[test]
    fn test_shared_buffer_cancel() {
        let buffer = SharedBuffer::new(100);
        assert!(!buffer.is_cancelled());

        buffer.cancel();
        assert!(buffer.is_cancelled());

        // Read should return error when cancelled
        let mut buf = [0u8; 5];
        let result = buffer.read_at(0, &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_shared_buffer_error() {
        let buffer = SharedBuffer::new(100);
        buffer.set_error("Test error".to_string());

        // Read should return error
        let mut buf = [0u8; 5];
        let result = buffer.read_at(0, &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_shared_buffer_progress() {
        let buffer = SharedBuffer::new(100);
        assert_eq!(buffer.progress(), 0.0);

        buffer.append(&[0; 25]);
        assert!((buffer.progress() - 0.25).abs() < 0.001);

        buffer.append(&[0; 25]);
        assert!((buffer.progress() - 0.50).abs() < 0.001);

        buffer.append(&[0; 50]);
        assert!((buffer.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_shared_buffer_set_total_size() {
        let buffer = SharedBuffer::new(0);
        assert_eq!(buffer.total_size(), 0);

        buffer.set_total_size(1000);
        assert_eq!(buffer.total_size(), 1000);
    }

    #[test]
    fn test_streaming_buffer_read() {
        let shared = SharedBuffer::new(100);
        shared.append(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let mut streaming = StreamingBuffer::new(shared);

        let mut buf = [0u8; 5];
        let bytes_read = streaming.read(&mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(buf, [1, 2, 3, 4, 5]);

        // Position should advance
        let bytes_read = streaming.read(&mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(buf, [6, 7, 8, 9, 10]);
    }

    #[test]
    fn test_streaming_buffer_seek() {
        let shared = SharedBuffer::new(100);
        shared.append(&[0; 50]);
        shared.set_total_size(100);

        let mut streaming = StreamingBuffer::new(shared);

        // Seek from start
        let pos = streaming.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(pos, 10);

        // Seek from current
        let pos = streaming.seek(SeekFrom::Current(5)).unwrap();
        assert_eq!(pos, 15);

        // Seek from current (negative)
        let pos = streaming.seek(SeekFrom::Current(-5)).unwrap();
        assert_eq!(pos, 10);

        // Seek from end
        let pos = streaming.seek(SeekFrom::End(-10)).unwrap();
        assert_eq!(pos, 90);
    }

    #[test]
    fn test_extract_extension_from_url() {
        assert_eq!(
            extract_extension_from_url("http://example.com/song.mp3"),
            Some("mp3".to_string())
        );
        assert_eq!(
            extract_extension_from_url("http://example.com/song.flac?token=xxx"),
            Some("flac".to_string())
        );
        assert_eq!(
            extract_extension_from_url("http://example.com/song.m4a#section"),
            Some("m4a".to_string())
        );
        assert_eq!(
            extract_extension_from_url("http://example.com/song.txt"),
            None
        );
        assert_eq!(extract_extension_from_url("http://example.com/song"), None);
    }

    #[test]
    fn test_content_type_to_extension() {
        assert_eq!(
            content_type_to_extension("audio/mpeg"),
            Some("mp3".to_string())
        );
        assert_eq!(
            content_type_to_extension("audio/flac"),
            Some("flac".to_string())
        );
        assert_eq!(
            content_type_to_extension("audio/mp4"),
            Some("m4a".to_string())
        );
        assert_eq!(
            content_type_to_extension("audio/ogg"),
            Some("ogg".to_string())
        );
        assert_eq!(
            content_type_to_extension("audio/wav"),
            Some("wav".to_string())
        );
        assert_eq!(content_type_to_extension("text/plain"), None);
        assert_eq!(
            content_type_to_extension("audio/mpeg; charset=utf-8"),
            Some("mp3".to_string())
        );
    }
}
