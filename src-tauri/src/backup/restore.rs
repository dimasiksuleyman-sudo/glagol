//! Backup validation + restore.
//!
//! Validation is split out as a separate entry point because the
//! frontend needs the manifest counts to render the «Восстановление»
//! confirmation modal *before* committing to the destructive restore.
//! Both functions are pure (take `&Path`s, no Tauri state) so they
//! drive cleanly from tests without spinning up a Tauri runtime.
//!
//! `restore_backup_impl` is intentionally one-shot: it re-runs
//! validation (defensive — never trust the frontend to skip steps),
//! creates a pre-restore safety snapshot of the *current* library
//! before any destructive work, then wipes the user's
//! `audio_cache/` + `glagol.db` and extracts the archive on top.
//!
//! The pre-restore zip is produced by reusing [`create::create_backup_impl`]
//! with the [`PRE_RESTORE_FILENAME_PREFIX`] filename prefix. If that
//! step fails (disk full, permission denied) the restore aborts
//! before any deletion happens, leaving the user's data exactly as
//! it was.

use std::fs::{self, File};
use std::io;
use std::path::Path;

use zip::ZipArchive;

use super::create::{create_backup_impl, read_manifest_from_zip};
use super::{
    BackupError, BackupManifest, BackupResult, AUDIO_DIR_PREFIX, CURRENT_BACKUP_VERSION,
    DB_FILENAME, MANIFEST_FILENAME, PRE_RESTORE_FILENAME_PREFIX,
};

/// Non-destructive pre-check. Opens `source_zip`, reads
/// `manifest.json`, runs the full validation chain, and returns the
/// parsed [`BackupManifest`] so the caller can render the user-facing
/// confirmation modal.
///
/// Roughly 50 ms on a typical library zip — the only I/O is the zip's
/// central directory and one inflated entry (the manifest itself).
pub fn validate_backup_impl(source_zip: &Path) -> BackupResult<BackupManifest> {
    let file = File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;

    // (1) Manifest must exist and parse.
    let manifest = read_manifest_from_zip(&mut archive)?;

    // (2) Reject archives newer than this build knows how to read.
    if manifest.backup_version > CURRENT_BACKUP_VERSION {
        return Err(BackupError::VersionUnsupported {
            found: manifest.backup_version,
            supported: CURRENT_BACKUP_VERSION,
        });
    }

    // (3) Manifest must be internally consistent. document_count and
    //     audio_file_count are joined 1:1 in the data model (per
    //     document a single WAV); a backup that claims otherwise is
    //     either corrupted or hand-crafted, and refusing both gives a
    //     cleaner mental model than silently picking one count.
    if manifest.document_count != manifest.audio_file_count {
        return Err(BackupError::ValidationFailed(format!(
            "Несоответствие счётчиков в manifest.json: {} документов, {} аудиофайлов.",
            manifest.document_count, manifest.audio_file_count
        )));
    }

    // (4) Walk the central directory once. Verify path-traversal
    //     safety on every entry (incl. unrelated ones), then count
    //     the audio entries + confirm `glagol.db` exists.
    let mut actual_audio_count: u32 = 0;
    let mut has_db = false;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name();
        if is_unsafe_entry_name(name) {
            return Err(BackupError::ValidationFailed(format!(
                "Архив содержит небезопасную запись: {name}"
            )));
        }
        if name == DB_FILENAME {
            has_db = true;
        } else if name.starts_with(AUDIO_DIR_PREFIX)
            && name
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            actual_audio_count = actual_audio_count.saturating_add(1);
        }
        // Other entries (manifest.json, plus any future or
        // user-introduced extras) are silently ignored at this stage —
        // they get reconfirmed safe by the traversal check above.
    }

    if !has_db {
        return Err(BackupError::ValidationFailed(
            "В архиве отсутствует glagol.db.".to_string(),
        ));
    }

    if actual_audio_count != manifest.audio_file_count {
        return Err(BackupError::ValidationFailed(format!(
            "Manifest заявляет {} аудиофайлов, в архиве найдено {}.",
            manifest.audio_file_count, actual_audio_count
        )));
    }

    Ok(manifest)
}

/// Replace the contents of `target_data_dir` with the contents of
/// `source_zip`. Caller is responsible for ensuring no other process
/// in this app holds the SQLite connection (see
/// `commands::backup::restore_backup` for the lock-release pattern).
///
/// Sequence: re-validate → create pre-restore safety zip in the same
/// folder as `source_zip` → wipe `audio_cache/` and `glagol.db` →
/// extract archive entries. Progress is emitted once per *user data*
/// entry (manifest is skipped; counts only `glagol.db` + audio
/// files), with `(0, total)` first and `(total, total)` last.
pub fn restore_backup_impl<F>(
    source_zip: &Path,
    target_data_dir: &Path,
    app_version: &str,
    progress_emit: F,
) -> BackupResult<()>
where
    F: Fn(u64, u64),
{
    // (1) Defensive re-validation. Frontend already called
    //     `validate_backup` but we never destroy user data based on
    //     trust in a previous caller.
    let manifest = validate_backup_impl(source_zip)?;

    // (2) Pre-restore safety net — D3 Safety Net 2. Lives in the
    //     same folder as the source backup so the user finds it next
    //     to their original file if they ever want to roll back.
    let safety_target = source_zip.parent().ok_or_else(|| {
        BackupError::ValidationFailed(
            "Не удалось определить папку для резервной копии текущего состояния.".to_string(),
        )
    })?;
    if let Err(e) = create_backup_impl(
        target_data_dir,
        safety_target,
        app_version,
        PRE_RESTORE_FILENAME_PREFIX,
        |_, _| {},
    ) {
        return Err(BackupError::PreRestoreBackupFailed {
            reason: e.to_string(),
        });
    }

    // (3) Wipe existing data. From this point on the user's library is
    //     gone; the pre-restore zip we just wrote is their recovery
    //     path if anything below fails.
    let audio_cache = target_data_dir.join("audio_cache");
    if audio_cache.exists() {
        fs::remove_dir_all(&audio_cache)?;
    }
    fs::create_dir_all(&audio_cache)?;
    let db_path = target_data_dir.join(DB_FILENAME);
    if db_path.exists() {
        fs::remove_file(&db_path)?;
    }

    // (4) Extract. Reopen the archive (the validation handle is
    //     dropped). The "user data" total — used for the progress
    //     callback — counts only entries we actually write back out:
    //     `glagol.db` + every audio file. Manifest is metadata and
    //     gets skipped during extraction.
    let file = File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;
    let total: u64 = u64::from(manifest.audio_file_count) + 1;
    progress_emit(0, total);

    let mut written: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if name == MANIFEST_FILENAME {
            continue;
        }

        // Path-traversal safety repeated here — defence in depth, even
        // though validation already passed.
        if is_unsafe_entry_name(&name) {
            return Err(BackupError::ValidationFailed(format!(
                "Архив содержит небезопасную запись: {name}"
            )));
        }

        let dest = if name == DB_FILENAME {
            target_data_dir.join(DB_FILENAME)
        } else if name.starts_with(AUDIO_DIR_PREFIX)
            && name
                .rsplit('.')
                .next()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
        {
            // Strip the prefix and reattach under the local
            // `audio_cache/` directory. Splitting on '/' guards
            // against weird relative segments after the prefix even
            // though `is_unsafe_entry_name` already rejected `..`.
            let basename = name
                .strip_prefix(AUDIO_DIR_PREFIX)
                .and_then(|s| s.rsplit('/').next())
                .ok_or_else(|| {
                    BackupError::ValidationFailed(format!("Некорректное имя записи: {name}"))
                })?;
            target_data_dir.join("audio_cache").join(basename)
        } else {
            // Unknown / unexpected entry — silently ignored per D6.
            continue;
        };

        let mut out = File::create(&dest)?;
        io::copy(&mut entry, &mut out)?;
        written = written.saturating_add(1);
        progress_emit(written.min(total), total);
    }

    Ok(())
}

/// Reject entries that could escape the target directory or attempt
/// to write to an absolute path. Standard "zip-slip" defence — see
/// e.g. <https://snyk.io/research/zip-slip-vulnerability>.
fn is_unsafe_entry_name(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    // Absolute paths (Unix and Windows backslash form).
    if name.starts_with('/') || name.starts_with('\\') {
        return true;
    }
    // Windows drive letter prefix, e.g. `C:\config.json`.
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return true;
    }
    // Any `..` component, regardless of separator.
    for segment in name.split(['/', '\\']) {
        if segment == ".." {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::create::create_backup_impl;
    use crate::backup::BACKUP_FILENAME_PREFIX;
    use crate::db;
    use crate::db::repository::{self, DocumentRecord};
    use std::io::Write;
    use std::path::PathBuf;
    use uuid::Uuid;
    use zip::write::{SimpleFileOptions, ZipWriter};
    use zip::CompressionMethod;

    fn fresh_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("glagol_restore_{label}_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create dir");
        dir
    }

    /// Build a zip on disk one entry at a time. `manifest_override` lets
    /// individual tests write either a valid manifest, a corrupted one,
    /// or omit it entirely (`None`).
    fn build_zip(
        path: &Path,
        manifest_bytes: Option<&[u8]>,
        include_db: bool,
        audio_filenames: &[&str],
    ) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        if let Some(bytes) = manifest_bytes {
            zip.start_file(MANIFEST_FILENAME, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        if include_db {
            zip.start_file(DB_FILENAME, opts).unwrap();
            zip.write_all(b"fake-db-bytes").unwrap();
        }
        for name in audio_filenames {
            zip.start_file(format!("{AUDIO_DIR_PREFIX}{name}"), stored)
                .unwrap();
            zip.write_all(b"fake-wav").unwrap();
        }
        zip.finish().unwrap();
    }

    fn seed_data_dir(doc_count: usize, audio_count: usize) -> PathBuf {
        let dir = fresh_dir("src");
        fs::create_dir_all(dir.join("audio_cache")).unwrap();
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
        }
        for i in 0..audio_count {
            let name = format!("{}-{i}.wav", Uuid::new_v4());
            fs::write(dir.join("audio_cache").join(&name), format!("audio-{i}")).unwrap();
        }
        dir
    }

    fn manifest_json(version: u32, doc_count: u32, audio_count: u32) -> String {
        let m = BackupManifest {
            backup_version: version,
            app_version: "0.1.0-test".to_string(),
            created_at: "2026-05-20T14:30:22Z".to_string(),
            document_count: doc_count,
            audio_file_count: audio_count,
            total_size_bytes: 1234,
        };
        serde_json::to_string_pretty(&m).unwrap()
    }

    #[test]
    fn validate_backup_rejects_missing_manifest() {
        let dir = fresh_dir("missing_manifest");
        let zip_path = dir.join("bad.zip");
        build_zip(&zip_path, None, true, &["a.wav"]);

        let err = validate_backup_impl(&zip_path).unwrap_err();
        match err {
            BackupError::ValidationFailed(msg) => {
                assert!(
                    msg.contains("manifest"),
                    "expected manifest-related reason, got: {msg}"
                );
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_backup_rejects_count_mismatch() {
        let dir = fresh_dir("count_mismatch");

        // External mismatch — manifest promises 5 audio files but the
        // archive only contains 3. This is the user-facing corruption
        // mode (files lost between archiving and now).
        let external = dir.join("external.zip");
        build_zip(
            &external,
            Some(manifest_json(1, 5, 5).as_bytes()),
            true,
            &["a.wav", "b.wav", "c.wav"],
        );
        let err = validate_backup_impl(&external).unwrap_err();
        assert!(
            matches!(err, BackupError::ValidationFailed(ref msg)
                if msg.contains("Manifest заявляет") && msg.contains('5') && msg.contains('3')),
            "external mismatch must surface manifest-vs-actual numbers, got {err:?}",
        );

        // Internal mismatch — manifest itself contradicts itself
        // (document_count != audio_file_count). Same error variant,
        // distinct message.
        let internal = dir.join("internal.zip");
        build_zip(
            &internal,
            Some(manifest_json(1, 3, 5).as_bytes()),
            true,
            &["a.wav", "b.wav", "c.wav", "d.wav", "e.wav"],
        );
        let err = validate_backup_impl(&internal).unwrap_err();
        assert!(
            matches!(err, BackupError::ValidationFailed(ref msg)
                if msg.contains("Несоответствие счётчиков")),
            "internal mismatch must surface счётчики message, got {err:?}",
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_backup_rejects_unknown_backup_version() {
        let dir = fresh_dir("version");
        let zip_path = dir.join("future.zip");
        build_zip(
            &zip_path,
            Some(manifest_json(99, 0, 0).as_bytes()),
            true,
            &[],
        );

        let err = validate_backup_impl(&zip_path).unwrap_err();
        match err {
            BackupError::VersionUnsupported { found, supported } => {
                assert_eq!(found, 99);
                assert_eq!(supported, CURRENT_BACKUP_VERSION);
            }
            other => panic!("expected VersionUnsupported, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_backup_rejects_path_traversal() {
        // Defence-in-depth: any entry whose name contains `..` or
        // tries to write to an absolute path must be refused. A
        // hostile archive could otherwise plant files anywhere the
        // app process can write.
        let dir = fresh_dir("traversal");
        let zip_path = dir.join("evil.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(MANIFEST_FILENAME, opts).unwrap();
        zip.write_all(manifest_json(1, 0, 0).as_bytes()).unwrap();
        zip.start_file(DB_FILENAME, opts).unwrap();
        zip.write_all(b"db").unwrap();
        zip.start_file("../../etc/passwd", opts).unwrap();
        zip.write_all(b"hostile").unwrap();
        zip.finish().unwrap();

        let err = validate_backup_impl(&zip_path).unwrap_err();
        assert!(
            matches!(err, BackupError::ValidationFailed(ref msg) if msg.contains("небезопасную")),
            "path traversal must be rejected, got {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_backup_replaces_existing_data_and_writes_safety_zip() {
        // End-to-end: take a real backup of dir A (5 docs / 5 audio),
        // mutate the data dir to look like dir B (3 docs / 3 audio),
        // restore from the original backup, then verify the data dir
        // now matches A *and* a pre-restore safety zip captured B.

        // Source library A — what we'll back up + later restore.
        let source_a = seed_data_dir(5, 5);
        let backup_target = fresh_dir("backup_target");
        let backup_zip = create_backup_impl(
            &source_a,
            &backup_target,
            "0.1.0-test",
            BACKUP_FILENAME_PREFIX,
            |_, _| {},
        )
        .expect("create backup A");

        // Live library — start at state B (3 docs / 3 audio).
        let live = seed_data_dir(3, 3);
        let live_audio_pre = count_wavs(&live.join("audio_cache"));
        assert_eq!(live_audio_pre, 3, "precondition: live has 3 wavs");

        // Restore from backup A on top of state B.
        restore_backup_impl(&backup_zip, &live, "0.1.0-test", |_, _| {}).expect("restore succeeds");

        // Live audio dir must now hold A's 5 files, not B's 3.
        let live_audio_post = count_wavs(&live.join("audio_cache"));
        assert_eq!(
            live_audio_post, 5,
            "live audio_cache must mirror backup A's 5 files after restore"
        );

        // Safety net: pre-restore zip lives next to the source zip,
        // and its manifest must remember B's pre-restore state.
        let safety_zip = find_pre_restore_zip(&backup_target)
            .expect("pre-restore safety zip should be created next to backup zip");
        let manifest = validate_backup_impl(&safety_zip).expect("safety zip is valid");
        assert_eq!(
            manifest.audio_file_count, 3,
            "pre-restore zip should capture B's 3 audio files, not A's 5"
        );

        let _ = fs::remove_dir_all(&source_a);
        let _ = fs::remove_dir_all(&backup_target);
        let _ = fs::remove_dir_all(&live);
    }

    fn count_wavs(dir: &Path) -> usize {
        fs::read_dir(dir)
            .map(|iter| {
                iter.filter_map(Result::ok)
                    .filter(|e| {
                        e.path()
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    fn find_pre_restore_zip(folder: &Path) -> Option<PathBuf> {
        fs::read_dir(folder).ok()?.find_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            (name.starts_with(PRE_RESTORE_FILENAME_PREFIX) && name.ends_with(".zip"))
                .then_some(path)
        })
    }
}
