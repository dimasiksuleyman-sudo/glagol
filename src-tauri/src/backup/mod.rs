//! Backup/restore: zip-based snapshot of the local library.
//!
//! Sprint 5c. A backup is a single `.zip` archive containing three
//! kinds of entries:
//!
//! ```text
//! glagol-backup-YYYY-MM-DD-HHMMSS.zip
//! ├── manifest.json                 (Deflated; tiny)
//! ├── glagol.db                     (Deflated; ~20-30% smaller)
//! └── audio_cache/{uuid}.wav        (Stored; WAV is already PCM)
//! ```
//!
//! `manifest.json` carries the structured metadata documented in
//! [`BackupManifest`]. It is the first entry the restore path reads —
//! validation runs against the manifest before any destructive work
//! starts, so a corrupt or foreign archive never gets a chance to
//! clobber the user's library.
//!
//! Per Sprint 5c Q1 the design optimises for portability/cloud-sync
//! rather than file size — audio files are stored uncompressed because
//! the CPU savings (5-10x) outweigh the marginal disk savings on
//! already-compressed PCM data.

pub mod error;

use serde::{Deserialize, Serialize};

pub use error::{BackupError, BackupResult};

/// The version of the manifest schema this build of Glagol can read
/// and write. Bumped whenever the on-disk shape of [`BackupManifest`]
/// changes incompatibly. Restore refuses archives whose
/// `backup_version` is **greater** than this constant; older versions
/// are read with explicit backward-compat handling at the call site
/// (none needed yet — Sprint 5c is v1).
pub const CURRENT_BACKUP_VERSION: u32 = 1;

/// Name of the manifest entry inside the archive. Single source of
/// truth referenced from both `create` and `restore`.
pub const MANIFEST_FILENAME: &str = "manifest.json";

/// Name of the database entry inside the archive.
pub const DB_FILENAME: &str = "glagol.db";

/// Directory prefix (with trailing slash) under which all `.wav`
/// entries live inside the archive. Restore validates that no entry
/// escapes this prefix via `..` segments — standard zip-slip defence.
pub const AUDIO_DIR_PREFIX: &str = "audio_cache/";

/// Structured metadata serialised as `manifest.json` at the root of
/// every Glagol backup archive.
///
/// Field semantics are locked by Sprint 5c D2. Counts are stored as
/// `u32` (4B+ documents would be remarkable for a single-user TTS
/// library) and the cumulative payload size as `u64` (audio libraries
/// routinely cross the 4 GiB mark).
///
/// `created_at` is an ISO 8601 UTC timestamp — local time goes into
/// the *filename* for human readability but the manifest stays UTC so
/// cross-timezone comparisons remain reliable when a backup travels
/// between machines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub backup_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub document_count: u32,
    pub audio_file_count: u32,
    pub total_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> BackupManifest {
        BackupManifest {
            backup_version: CURRENT_BACKUP_VERSION,
            app_version: "0.1.0-rc.5".to_string(),
            created_at: "2026-05-20T14:30:22Z".to_string(),
            document_count: 42,
            audio_file_count: 42,
            total_size_bytes: 87_654_321,
        }
    }

    #[test]
    fn manifest_round_trip_preserves_all_fields() {
        let original = sample_manifest();
        let json = serde_json::to_string_pretty(&original).expect("serialise");
        let restored: BackupManifest = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(restored, original);
    }

    #[test]
    fn manifest_json_is_pretty_printed_and_field_named() {
        // Pretty-printed JSON exists explicitly for human debugging
        // (D2 Q5). Lock the field naming convention (snake_case via
        // serde defaults) so a future #[serde(rename_all = …)] tweak
        // doesn't silently break older backups.
        let json = serde_json::to_string_pretty(&sample_manifest()).expect("serialise");
        assert!(json.contains('\n'), "manifest must be pretty-printed");
        assert!(json.contains("\"backup_version\""));
        assert!(json.contains("\"app_version\""));
        assert!(json.contains("\"created_at\""));
        assert!(json.contains("\"document_count\""));
        assert!(json.contains("\"audio_file_count\""));
        assert!(json.contains("\"total_size_bytes\""));
    }
}
