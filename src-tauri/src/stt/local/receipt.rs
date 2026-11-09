//! Reuse of a full GigaAM verification (0.6.2).
//!
//! The first load after start used to hash the ~272 MB model and re-read the
//! runtime archive every time (~3 s on an HDD PC). Like Silero, a full check is
//! now reused for at most [`MAX_AGE_SECS`] while the size and modification time
//! of every key file are unchanged. A missing, stale or mismatching receipt, a
//! failed session open, a recognition error, a repair or a removal force the
//! full SHA-256 check again.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A receipt is trusted for 30 days.
pub const MAX_AGE_SECS: i64 = 30 * 24 * 60 * 60;

/// Size and modification time of one verified file, by path relative to root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub path: String,
    pub bytes: u64,
    pub modified_ns: u64,
}

/// What a full verification observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub verified_at: i64,
    pub files: Vec<FileStamp>,
}

fn receipt_path(root: &Path, model_id: &str) -> PathBuf {
    root.join(format!("verification-{model_id}.json"))
}

fn stamp(root: &Path, relative: &Path) -> Option<FileStamp> {
    let metadata = std::fs::symlink_metadata(root.join(relative)).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified_ns = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .min(u64::MAX as u128) as u64;
    Some(FileStamp {
        path: relative.to_string_lossy().replace('\\', "/"),
        bytes: metadata.len(),
        modified_ns,
    })
}

/// Stamps of the model, the runtime archive and every runtime file, sorted.
/// `None` when any expected file is missing or is not a regular file.
pub fn current_files(
    root: &Path,
    model_file: &str,
    archive: &str,
    runtime_dir: &str,
) -> Option<Vec<FileStamp>> {
    let mut files = vec![
        stamp(root, Path::new(model_file))?,
        stamp(root, Path::new(archive))?,
    ];
    let mut pending = vec![PathBuf::from(runtime_dir)];
    let mut runtime_files = 0usize;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(root.join(&dir)).ok()? {
            let entry = entry.ok()?;
            let relative = dir.join(entry.file_name());
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                pending.push(relative);
            } else if kind.is_file() {
                files.push(stamp(root, &relative)?);
                runtime_files += 1;
            } else {
                return None;
            }
        }
    }
    if runtime_files == 0 {
        return None;
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Some(files)
}

/// Whether a saved receipt is recent and matches `files` exactly.
pub fn is_valid(receipt: &Receipt, files: &[FileStamp], now: i64) -> bool {
    let age = now - receipt.verified_at;
    (0..MAX_AGE_SECS).contains(&age) && receipt.files == files
}

/// Read the saved receipt for `model_id`, if any.
pub fn load(root: &Path, model_id: &str) -> Option<Receipt> {
    let text = std::fs::read_to_string(receipt_path(root, model_id)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Save a receipt after a successful full verification. Best-effort.
pub fn save(root: &Path, model_id: &str, files: Vec<FileStamp>) {
    let receipt = Receipt {
        verified_at: chrono::Utc::now().timestamp(),
        files,
    };
    let saved = serde_json::to_vec(&receipt)
        .ok()
        .and_then(|bytes| std::fs::write(receipt_path(root, model_id), bytes).ok());
    if saved.is_none() {
        tracing::warn!(
            model = model_id,
            "local STT verification receipt could not be saved"
        );
    }
}

/// Force the next load of `model_id` to run the full verification.
pub fn invalidate(root: &Path, model_id: &str) {
    match std::fs::remove_file(receipt_path(root, model_id)) {
        Ok(()) => tracing::info!(model = model_id, "local STT verification receipt cleared"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => tracing::warn!(
            model = model_id,
            "local STT verification receipt could not be cleared"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated directory removed on drop.
    struct Dir(PathBuf);
    impl Dir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn layout() -> Dir {
        let dir = Dir(std::env::temp_dir().join(format!(
            "glagol_stt_receipt_{}",
            uuid::Uuid::new_v4().simple()
        )));
        std::fs::create_dir_all(dir.path().join("rt/licenses")).unwrap();
        std::fs::write(dir.path().join("model.gguf"), b"model").unwrap();
        std::fs::write(dir.path().join("runtime.tar.gz"), b"archive").unwrap();
        std::fs::write(dir.path().join("rt/transcribe.dll"), b"dll").unwrap();
        std::fs::write(dir.path().join("rt/licenses/LICENSE"), b"mit").unwrap();
        dir
    }

    #[test]
    fn receipt_is_reused_only_while_files_and_age_match() {
        let dir = layout();
        let root = dir.path();
        let files = current_files(root, "model.gguf", "runtime.tar.gz", "rt").unwrap();
        assert_eq!(files.len(), 4);
        assert!(files.windows(2).all(|w| w[0].path < w[1].path));
        save(root, "m", files.clone());
        let receipt = load(root, "m").unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(is_valid(&receipt, &files, now));
        assert!(!is_valid(&receipt, &files, now + MAX_AGE_SECS + 1));
        assert!(!is_valid(&receipt, &files, receipt.verified_at - 1));

        // A changed runtime file (size differs) invalidates the receipt.
        std::fs::write(root.join("rt/transcribe.dll"), b"tampered dll").unwrap();
        let changed = current_files(root, "model.gguf", "runtime.tar.gz", "rt").unwrap();
        assert!(!is_valid(&receipt, &changed, now));

        invalidate(root, "m");
        assert!(load(root, "m").is_none());
    }

    #[test]
    fn missing_files_or_empty_runtime_never_produce_stamps() {
        let dir = layout();
        let root = dir.path();
        assert!(current_files(root, "absent.gguf", "runtime.tar.gz", "rt").is_none());
        std::fs::remove_file(root.join("rt/transcribe.dll")).unwrap();
        std::fs::remove_file(root.join("rt/licenses/LICENSE")).unwrap();
        assert!(current_files(root, "model.gguf", "runtime.tar.gz", "rt").is_none());
    }
}
