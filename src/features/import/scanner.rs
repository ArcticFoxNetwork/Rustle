//! Recursive folder scanner with parallel processing
//!
//! Scans directories for audio files, extracts metadata in parallel,
//! and reports progress via channels.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use rayon::prelude::*;
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

use super::cover::CoverCache;
use super::is_audio_file;
use super::metadata::{AudioMetadata, apply_smart_parsing, extract_metadata, resolve_track_gain};
use super::progress::{ProgressSender, ScanProgress, ScanState, SkipReason};
use crate::database::{Database, NewSong};

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Whether to compute file hashes for deduplication
    pub compute_hash: bool,
    /// Whether to extract and cache cover art
    pub extract_covers: bool,
    /// Whether to apply smart filename parsing
    pub smart_parsing: bool,
    /// Maximum depth to scan (None = unlimited)
    pub max_depth: Option<usize>,
    /// File extensions to include (empty = all supported)
    pub extensions: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            compute_hash: true,
            extract_covers: true,
            smart_parsing: true,
            max_depth: None,
            extensions: Vec::new(),
        }
    }
}

/// Result of scanning a single file
#[derive(Debug)]
pub struct ScanResult {
    pub metadata: AudioMetadata,
    pub file_size: u64,
    pub file_hash: Option<String>,
    pub normalization_gain: Option<f64>,
    pub cover_path: Option<PathBuf>,
}

struct PendingImport {
    path: PathBuf,
    file_name: String,
    title: String,
    artist: String,
    cover_path: Option<String>,
}

/// Scan a directory for audio files
///
/// Returns a list of audio file paths found
pub fn discover_audio_files(root: &Path, config: &ScanConfig) -> Vec<PathBuf> {
    let mut walker = WalkDir::new(root).follow_links(true).into_iter();

    if let Some(max_depth) = config.max_depth {
        walker = WalkDir::new(root)
            .max_depth(max_depth)
            .follow_links(true)
            .into_iter();
    }

    walker
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| {
            if !config.extensions.is_empty() {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        config
                            .extensions
                            .iter()
                            .any(|ext| ext.eq_ignore_ascii_case(e))
                    })
                    .unwrap_or(false)
            } else {
                is_audio_file(p)
            }
        })
        .collect()
}

/// 计算部分文件哈希（前 64KB + 后 64KB + 文件大小）
/// 对大文件更快，同时保持合理的唯一性
pub fn compute_partial_hash(path: &Path) -> Result<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    const CHUNK_SIZE: usize = 64 * 1024; // 64KB
    let mut hasher_data = Vec::with_capacity(CHUNK_SIZE * 2 + 8);

    // Add file size to hash
    hasher_data.extend_from_slice(&file_size.to_le_bytes());

    // Read first chunk
    let mut first_chunk = vec![0u8; CHUNK_SIZE.min(file_size as usize)];
    file.read_exact(&mut first_chunk)?;
    hasher_data.extend_from_slice(&first_chunk);

    // Read last chunk if file is large enough
    if file_size > CHUNK_SIZE as u64 * 2 {
        file.seek(SeekFrom::End(-(CHUNK_SIZE as i64)))?;
        let mut last_chunk = vec![0u8; CHUNK_SIZE];
        file.read_exact(&mut last_chunk)?;
        hasher_data.extend_from_slice(&last_chunk);
    }

    Ok(format!("{:016x}", xxh3_64(&hasher_data)))
}

/// Check for external cover art files (same-name image or common cover files)
fn check_external_cover(audio_path: &Path) -> Option<PathBuf> {
    use crate::features::media::cover;

    // Try to find external cover file
    if let Some(cover_source) = cover::find_cover_art(audio_path)
        && let Some(path) = cover_source.path()
    {
        return Some(path.to_path_buf());
    }

    None
}

fn classify_metadata_error(path: &Path, error: anyhow::Error) -> SkipReason {
    if has_known_audio_header(path).unwrap_or(false) {
        SkipReason::Corrupted
    } else if is_audio_file(path) {
        SkipReason::NotAudioFile
    } else {
        SkipReason::MetadataError(error.to_string())
    }
}

fn has_known_audio_header(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut header = [0_u8; 64];
    let len = file.read(&mut header)?;
    let bytes = &header[..len];
    Ok(bytes.starts_with(b"fLaC")
        || bytes.starts_with(b"ID3")
        || matches!(
            bytes.get(0..2),
            Some([0xFF, 0xFB] | [0xFF, 0xFA] | [0xFF, 0xF3] | [0xFF, 0xF2])
        )
        || bytes.starts_with(b"OggS")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE")
        || (bytes.len() >= 8 && &bytes[4..8] == b"ftyp"))
}

/// Process a single audio file.
pub fn scan_audio_file(
    path: &Path,
    config: &ScanConfig,
    cover_cache: Option<&CoverCache>,
) -> Result<ScanResult> {
    if !is_audio_file(path) {
        return Err(SkipReason::NotAudioFile.into());
    }

    // Get file metadata
    let file_meta =
        std::fs::metadata(path).map_err(|_| anyhow::Error::from(SkipReason::Corrupted))?;

    // Skip empty files
    if file_meta.len() == 0 {
        return Err(SkipReason::EmptyFile.into());
    }

    // Extract audio metadata
    let mut metadata = match extract_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return Err(classify_metadata_error(path, error).into()),
    };

    // Apply smart filename parsing if enabled
    if config.smart_parsing
        && let Some(filename) = path.file_name().and_then(|n| n.to_str())
    {
        apply_smart_parsing(&mut metadata, filename);
    }

    // Compute file hash if enabled
    let file_hash = if config.compute_hash {
        Some(compute_partial_hash(path)?)
    } else {
        None
    };

    let normalization_gain = resolve_track_gain(path).map(f64::from);

    // Process cover art if enabled
    // Priority: embedded > same-name image > common cover files
    let cover_path = if config.extract_covers {
        // First try embedded cover art
        if let (Some(cover_data), Some(cache)) = (&metadata.cover_data, cover_cache) {
            match cache.save_cover_with_mime(cover_data, metadata.cover_mime.as_deref()) {
                Ok((_hash, path)) => Some(path),
                Err(e) => {
                    tracing::warn!("Failed to cache cover for {:?}: {}", path, e);
                    // Fall through to external file check
                    check_external_cover(path)
                }
            }
        } else {
            // No embedded cover, check for external files
            check_external_cover(path)
        }
    } else {
        None
    };

    Ok(ScanResult {
        metadata,
        file_size: file_meta.len(),
        file_hash,
        normalization_gain,
        cover_path,
    })
}

/// 扫描目录并导入歌曲到数据库
///
/// 文件夹扫描的主入口
/// It runs in a background task and reports progress via the channel.
pub async fn scan_and_import(
    db: Arc<Database>,
    root: PathBuf,
    config: ScanConfig,
    cover_cache: Arc<CoverCache>,
    state: Arc<ScanState>,
    progress_tx: ProgressSender,
) -> Result<()> {
    let start_time = Instant::now();

    // Discover all files that match the current scan filter.
    let files = tokio::task::spawn_blocking({
        let root = root.clone();
        let config = config.clone();
        move || discover_audio_files(&root, &config)
    })
    .await?;

    let total_files = files.len() as u64;
    state.set_total(total_files);
    state.set_scanned_paths(Vec::new());

    let _ = progress_tx.send(ScanProgress::Started { total_files });

    if total_files == 0 {
        let _ = progress_tx.send(ScanProgress::Completed {
            imported: 0,
            skipped: 0,
            errors: 0,
            duration_secs: start_time.elapsed().as_secs_f64(),
        });
        return Ok(());
    }

    // Process files in parallel batches
    let batch_size = 100;
    for batch in files.chunks(batch_size) {
        if state.is_cancelled() {
            let _ = progress_tx.send(ScanProgress::Cancelled);
            return Ok(());
        }

        let batch: Vec<PathBuf> = batch.to_vec();
        let config = config.clone();
        let cover_cache = cover_cache.clone();

        // Process batch in parallel using rayon
        let results: Vec<(PathBuf, Result<ScanResult>)> = tokio::task::spawn_blocking(move || {
            batch
                .par_iter()
                .map(|path| {
                    let result = scan_audio_file(path, &config, Some(&cover_cache));
                    (path.clone(), result)
                })
                .collect()
        })
        .await?;

        let mut pending_imports = Vec::new();
        let mut pending_songs = Vec::new();

        // Prepare database rows while preserving per-file progress updates.
        for (path, result) in results {
            if state.is_cancelled() {
                let _ = progress_tx.send(ScanProgress::Cancelled);
                return Ok(());
            }

            let current = state.increment_current();
            let path_str = path.to_string_lossy().to_string();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let _ = progress_tx.send(ScanProgress::Processing {
                current,
                total: total_files,
                file_name: file_name.clone(),
            });

            match result {
                Ok(scan_result) => {
                    let cover_path_str = scan_result
                        .cover_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string());

                    let new_song = NewSong {
                        file_path: path_str,
                        title: scan_result.metadata.title.clone(),
                        artist: scan_result.metadata.artist.clone(),
                        album: scan_result.metadata.album.clone(),
                        duration_secs: scan_result.metadata.duration_secs,
                        track_number: scan_result.metadata.track_number,
                        year: scan_result.metadata.year,
                        genre: scan_result.metadata.genre.clone(),
                        cover_path: cover_path_str.clone(),
                        file_hash: scan_result.file_hash,
                        file_size: scan_result.file_size as i64,
                        format: Some(scan_result.metadata.format),
                        normalization_gain: scan_result.normalization_gain,
                    };

                    pending_imports.push(PendingImport {
                        path,
                        file_name,
                        title: scan_result.metadata.title,
                        artist: scan_result.metadata.artist,
                        cover_path: cover_path_str,
                    });
                    pending_songs.push(new_song);
                }
                Err(e) => {
                    state.increment_skipped();
                    let reason = e
                        .downcast::<SkipReason>()
                        .unwrap_or_else(|e| SkipReason::MetadataError(e.to_string()));
                    let _ = progress_tx.send(ScanProgress::Skipped {
                        current,
                        total: total_files,
                        file_name,
                        reason,
                    });
                }
            }
        }

        if pending_songs.is_empty() {
            continue;
        }

        if state.is_cancelled() {
            let _ = progress_tx.send(ScanProgress::Cancelled);
            return Ok(());
        }

        match db.upsert_local_songs(pending_songs).await {
            Ok(ids) if ids.len() == pending_imports.len() => {
                for pending in pending_imports {
                    if state.is_cancelled() {
                        let _ = progress_tx.send(ScanProgress::Cancelled);
                        return Ok(());
                    }

                    state.increment_imported();
                    state.push_scanned_path(pending.path);
                    let (_, current, _, _, _) = state.get_stats();
                    let _ = progress_tx.send(ScanProgress::Imported {
                        current,
                        total: total_files,
                        title: pending.title,
                        artist: pending.artist,
                        cover_path: pending.cover_path,
                    });
                }
            }
            Ok(ids) => {
                let message = format!(
                    "batch upsert returned {} ids for {} songs",
                    ids.len(),
                    pending_imports.len()
                );
                for pending in pending_imports {
                    state.increment_errors();
                    let (_, current, _, _, _) = state.get_stats();
                    let _ = progress_tx.send(ScanProgress::Skipped {
                        current,
                        total: total_files,
                        file_name: pending.file_name,
                        reason: SkipReason::DatabaseError(message.clone()),
                    });
                }
            }
            Err(e) => {
                let message = e.to_string();
                for pending in pending_imports {
                    state.increment_errors();
                    let (_, current, _, _, _) = state.get_stats();
                    let _ = progress_tx.send(ScanProgress::Skipped {
                        current,
                        total: total_files,
                        file_name: pending.file_name,
                        reason: SkipReason::DatabaseError(message.clone()),
                    });
                }
            }
        }
    }

    let (_, _, imported, skipped, errors) = state.get_stats();
    let _ = progress_tx.send(ScanProgress::Completed {
        imported,
        skipped,
        errors,
        duration_secs: start_time.elapsed().as_secs_f64(),
    });

    Ok(())
}
