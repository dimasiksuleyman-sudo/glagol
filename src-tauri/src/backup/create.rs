//! Backup creation: archive `glagol.db` and every `audio_cache/*.wav`
//! into a single zip, alongside a manifest describing the contents.
//!
//! All work runs synchronously inside one call; the Tauri command layer
//! wraps this in `spawn_blocking` so the UI thread keeps moving. The
//! impl takes a `progress_emit` callback rather than reaching into a
//! Tauri `AppHandle` directly — that keeps it testable without a
//! running Tauri runtime, in line with the `*_impl` pattern already
//! established for `synthesize_document` and friends.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Local, Utc};
use rusqlite::{Connection, OpenFlags};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use super::{
    BackupError, BackupManifest, BackupResult, AUDIO_DIR_PREFIX, CURRENT_BACKUP_VERSION,
    DB_FILENAME, MANIFEST_FILENAME,
};

/// Write a full backup of `source_data_dir` into `target_folder`.
///
/// `source_data_dir` is the directory that contains `glagol.db` and the
/// `audio_cache/` subdirectory (`%LOCALAPPDATA%\app.glagol.desktop\` in
/// production). `target_folder` is a user-picked directory the archive
/// is written into; it must already exist and be writable.
///
/// `progress_emit` is invoked `2 + audio_file_count` times — once each
/// after the manifest, the database, and each audio file is written —
/// with `(current, total)` so the caller can drive a progress modal.
/// The very first call is `(0, total)` so the modal can render its
/// initial "0 of N" frame before any zip work starts.
///
/// On success the absolute path of the freshly created zip is returned.
pub fn create_backup_impl<F>(
    source_data_dir: &Path,
    target_folder: &Path,
    app_version: &str,
    filename_prefix: &str,
    progress_emit: F,
) -> BackupResult<PathBuf>
where
    F: Fn(u64, u64),
{
    validate_target_folder(target_folder)?;

    let audio_dir = source_data_dir.join("audio_cache");
    let audio_files = enumerate_audio_files(&audio_dir)?;
    let audio_file_count: u32 = audio_files.len().try_into().map_err(|_| {
        BackupError::ValidationFailed(
            "Слишком много аудиофайлов для одной резервной копии.".to_string(),
        )
    })?;

    let db_path = source_data_dir.join(DB_FILENAME);
    let document_count = count_documents(&db_path)?;

    let total_size_bytes = sum_payload_size(&db_path, &audio_files)?;

    let manifest = BackupManifest {
        backup_version: CURRENT_BACKUP_VERSION,
        app_version: app_version.to_string(),
        created_at: Utc::now().to_rfc3339(),
        document_count,
        audio_file_count,
        total_size_bytes,
    };

    let filename = format!(
        "{filename_prefix}-{}.zip",
        Local::now().format("%Y-%m-%d-%H%M%S")
    );
    let target_path = target_folder.join(&filename);

    // Total = manifest + db + every audio file. Cast through u64 here
    // so the progress callback signature stays consistent regardless
    // of audio file count's u32 origin.
    let total: u64 = u64::from(audio_file_count) + 2;
    progress_emit(0, total);

    let zip_file = File::create(&target_path)?;
    let mut zip = ZipWriter::new(zip_file);

    write_manifest_entry(&mut zip, &manifest)?;
    progress_emit(1, total);

    write_db_entry(&mut zip, &db_path)?;
    progress_emit(2, total);

    for (index, audio_path) in audio_files.iter().enumerate() {
        write_audio_entry(&mut zip, audio_path)?;
        // 1-based audio index + 2 for the manifest and db entries
        // already written. The cast is safe because audio_files.len()
        // already fit into a u32 above.
        progress_emit(index as u64 + 3, total);
    }

    zip.finish()?;
    Ok(target_path)
}

// ── Internals ──────────────────────────────────────────────────────────

fn validate_target_folder(folder: &Path) -> BackupResult<()> {
    if !folder.exists() {
        return Err(BackupError::ValidationFailed(format!(
            "Папка для резервной копии не существует: {}",
            folder.display()
        )));
    }
    if !folder.is_dir() {
        return Err(BackupError::ValidationFailed(format!(
            "Указанный путь не является папкой: {}",
            folder.display()
        )));
    }
    // Probe write access by touching a sentinel file. Same pattern used
    // for the (now removed) Sprint 5b library-path validator; the only
    // truly reliable cross-platform writability check is "try to write".
    let probe = folder.join("_glagol_writable_check.tmp");
    match File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(BackupError::ValidationFailed(format!(
            "Папка недоступна для записи: {e}"
        ))),
    }
}

/// Collect every `*.wav` file directly under `audio_dir`. Returns an
/// empty `Vec` if the directory does not exist (fresh install with
/// nothing synthesised yet) — that is a valid backup, not an error.
/// The list is sorted by path so backup contents are deterministic
/// across runs, which makes tests sane and diffs across two backups of
/// the same library byte-identical for the audio portion.
fn enumerate_audio_files(audio_dir: &Path) -> BackupResult<Vec<PathBuf>> {
    if !audio_dir.exists() {
        return Ok(Vec::new());
    }
    let mut wavs: Vec<PathBuf> = fs::read_dir(audio_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        })
        .collect();
    wavs.sort();
    Ok(wavs)
}

fn count_documents(db_path: &Path) -> BackupResult<u32> {
    if !db_path.exists() {
        // A library directory without `glagol.db` (corrupted install,
        // user moved files manually). The schema is restored on next
        // app launch; for backup purposes, report zero documents.
        return Ok(0);
    }
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| BackupError::ValidationFailed(format!("Не удалось открыть базу данных: {e}")))?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
        .map_err(|e| {
            BackupError::ValidationFailed(format!("Не удалось прочитать список документов: {e}"))
        })?;
    u32::try_from(count.max(0)).map_err(|_| {
        BackupError::ValidationFailed("Слишком много документов в базе данных.".to_string())
    })
}

fn sum_payload_size(db_path: &Path, audio_files: &[PathBuf]) -> BackupResult<u64> {
    let mut total: u64 = 0;
    if db_path.exists() {
        total = total.saturating_add(fs::metadata(db_path)?.len());
    }
    for path in audio_files {
        total = total.saturating_add(fs::metadata(path)?.len());
    }
    Ok(total)
}

fn write_manifest_entry<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    manifest: &BackupManifest,
) -> BackupResult<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    zip.start_file(
        MANIFEST_FILENAME,
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644),
    )?;
    zip.write_all(json.as_bytes())?;
    Ok(())
}

fn write_db_entry<W: Write + io::Seek>(zip: &mut ZipWriter<W>, db_path: &Path) -> BackupResult<()> {
    zip.start_file(
        DB_FILENAME,
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644),
    )?;
    if db_path.exists() {
        let mut file = File::open(db_path)?;
        io::copy(&mut file, zip)?;
    }
    Ok(())
}

fn write_audio_entry<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    audio_path: &Path,
) -> BackupResult<()> {
    let filename = audio_path
        .file_name()
        .ok_or_else(|| BackupError::ValidationFailed("Аудиофайл без имени.".to_string()))?
        .to_string_lossy()
        .into_owned();
    let entry_name = format!("{AUDIO_DIR_PREFIX}{filename}");
    zip.start_file(
        entry_name,
        SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644),
    )?;
    let mut file = File::open(audio_path)?;
    io::copy(&mut file, zip)?;
    Ok(())
}

// Helper used by tests + restore-path validation: parse the manifest
// JSON out of a generic byte reader. Kept here next to the writer so
// the field naming stays in lock-step.
pub(crate) fn parse_manifest_from_bytes(bytes: &[u8]) -> BackupResult<BackupManifest> {
    let manifest: BackupManifest = serde_json::from_slice(bytes)?;
    Ok(manifest)
}

pub(crate) fn read_manifest_from_zip<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> BackupResult<BackupManifest> {
    let mut entry = archive.by_name(MANIFEST_FILENAME).map_err(|_| {
        BackupError::ValidationFailed(
            "Этот файл не является корректной резервной копией Glagol (отсутствует manifest.json)."
                .to_string(),
        )
    })?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    parse_manifest_from_bytes(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::db::repository::{self, DocumentRecord};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Lay out a fake `%LOCALAPPDATA%\app.glagol.desktop\` directory:
    /// real migrated `glagol.db` populated with `doc_count` rows, plus
    /// `audio_cache/{uuid}.wav` files containing arbitrary bytes (the
    /// archive doesn't care about WAV validity — restore extracts them
    /// byte-for-byte).
    fn seed_data_dir(doc_count: usize, audio_count: usize) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glagol_backup_src_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create source dir");
        fs::create_dir_all(dir.join("audio_cache")).expect("create audio_cache");

        let db_path = dir.join("glagol.db");
        {
            let conn = db::init_database(&db_path).expect("init db");
            for i in 0..doc_count {
                let record = DocumentRecord {
                    id: Uuid::new_v4().to_string(),
                    title: format!("Документ {i}"),
                    source_type: "paste".to_string(),
                    char_count: 100,
                    voice: "Nec_24000".to_string(),
                    status: "ready".to_string(),
                    error_message: None,
                    created_at: 1_700_000_000_000 + i as i64,
                    audio_path: Some(format!("{}.wav", Uuid::new_v4())),
                    audio_duration_ms: Some(1234),
                };
                repository::insert(&conn, &record).expect("insert");
            }
            drop(conn);
        }

        for i in 0..audio_count {
            let path = dir
                .join("audio_cache")
                .join(format!("{}-{i}.wav", Uuid::new_v4()));
            // Distinct payloads so restore tests can prove which file
            // came from which source.
            let payload = format!("fake-wav-payload-{i}").into_bytes();
            fs::write(&path, &payload).expect("write wav");
        }

        dir
    }

    fn fresh_target_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glagol_backup_dst_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create target dir");
        dir
    }

    #[test]
    fn create_backup_writes_valid_zip() {
        let source = seed_data_dir(2, 2);
        let target = fresh_target_dir();

        let path = create_backup_impl(
            &source,
            &target,
            "0.1.0-test",
            BACKUP_FILENAME_PREFIX,
            |_, _| {},
        )
        .expect("backup succeeds");
        assert!(path.exists(), "zip file must exist on disk");
        assert!(
            path.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("glagol-backup-") && s.ends_with(".zip")),
            "filename pattern: {}",
            path.display()
        );

        let file = File::open(&path).expect("open zip");
        let archive = zip::ZipArchive::new(file).expect("archive parses");
        assert!(
            archive.len() >= 3,
            "archive must contain at least manifest + db + audio entries"
        );

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&target);
    }

    #[test]
    fn create_backup_includes_manifest_db_and_audio() {
        let source = seed_data_dir(3, 3);
        let target = fresh_target_dir();

        let path = create_backup_impl(
            &source,
            &target,
            "0.1.0-test",
            BACKUP_FILENAME_PREFIX,
            |_, _| {},
        )
        .expect("backup succeeds");

        let file = File::open(&path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("archive parses");

        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(names.iter().any(|n| n == MANIFEST_FILENAME));
        assert!(names.iter().any(|n| n == DB_FILENAME));
        let audio_entries: Vec<&String> = names
            .iter()
            .filter(|n| n.starts_with(AUDIO_DIR_PREFIX) && n.ends_with(".wav"))
            .collect();
        assert_eq!(
            audio_entries.len(),
            3,
            "expected 3 wav entries, got: {names:?}"
        );
        assert_eq!(archive.len(), 5, "manifest + db + 3 audio = 5 entries");

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&target);
    }

    #[test]
    fn create_backup_manifest_counts_match_inputs() {
        let source = seed_data_dir(3, 3);
        let target = fresh_target_dir();

        // Capture progress events so we can sanity-check the cadence
        // (first call (0, total), final call (total, total)).
        let progress: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(Vec::new()));
        let progress_clone = Arc::clone(&progress);

        let path = create_backup_impl(
            &source,
            &target,
            "0.1.0-rc.5",
            BACKUP_FILENAME_PREFIX,
            move |c, t| {
                progress_clone.lock().unwrap().push((c, t));
            },
        )
        .expect("backup succeeds");

        let file = File::open(&path).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("archive parses");

        // Manifest extraction via the same helper restore will use.
        let manifest = read_manifest_from_zip(&mut archive).expect("read manifest");
        assert_eq!(manifest.backup_version, CURRENT_BACKUP_VERSION);
        assert_eq!(manifest.app_version, "0.1.0-rc.5");
        assert_eq!(manifest.document_count, 3);
        assert_eq!(manifest.audio_file_count, 3);
        assert!(
            manifest.total_size_bytes > 0,
            "total_size_bytes must be the sum of db + audio sizes"
        );
        assert!(
            manifest.created_at.contains('T') && manifest.created_at.ends_with('Z')
                || manifest.created_at.contains("+00:00"),
            "created_at must be ISO 8601 UTC, got: {}",
            manifest.created_at
        );

        // Progress cadence: first (0, 5), last (5, 5) — 5 = manifest + db + 3 audio.
        let events = progress.lock().unwrap();
        assert_eq!(events.first().copied(), Some((0, 5)));
        assert_eq!(events.last().copied(), Some((5, 5)));

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&target);
    }
}
