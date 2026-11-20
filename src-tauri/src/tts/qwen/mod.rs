//! Optional Qwen3-TTS (series tts-qwen, step Q3): Apache 2.0 models and the
//! MIT qwentts.cpp runtime, downloaded on demand from pinned URLs, verified
//! by SHA-256 before anything is loaded, run in a hidden worker process.
pub mod catalog;
mod engine;
pub mod worker;
use crate::stt::local::{catalog::Artifact, download};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

/// Named speakers of the CustomVoice checkpoint (the model's own order).
pub const VOICES: &[&str] = &[
    "ryan", "aiden", "serena", "vivian", "uncle_fu", "ono_anna", "sohee", "eric", "dylan",
];
/// Licenses shown in Settings and stored beside the files.
pub const LICENSE: &str = include_str!("../../../../docs/third-party/Qwen3-TTS-LICENSE.txt");

/// Every file Glagol loads, with its pinned hash: models and runtime DLLs.
fn pinned() -> impl Iterator<Item = &'static Artifact> {
    [&catalog::TALKER, &catalog::CODEC]
        .into_iter()
        .chain(catalog::DLLS.iter())
}

/// Full verification of every model and runtime file.
pub fn verify(root: &Path) -> Result<(), String> {
    for artifact in pinned() {
        download::verify(&root.join(artifact.file), artifact)?;
    }
    Ok(())
}

/// Whether all files are present with the expected sizes (no hashing).
pub fn installed(root: &Path) -> bool {
    pinned().all(|a| std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes))
}

/// Bytes still to download (resumable parts count as downloaded).
pub fn missing_bytes(root: &Path) -> u64 {
    let runtime_ready = catalog::DLLS
        .iter()
        .all(|a| std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes));
    [&catalog::TALKER, &catalog::CODEC]
        .into_iter()
        .chain((!runtime_ready).then_some(&catalog::RUNTIME))
        .map(|a| {
            if std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes) {
                return 0;
            }
            a.bytes.saturating_sub(
                std::fs::metadata(root.join(format!("{}.part", a.file)))
                    .map(|m| m.len().min(a.bytes))
                    .unwrap_or(0),
            )
        })
        .sum()
}

/// Bytes of partial downloads.
pub fn partial_bytes(root: &Path) -> u64 {
    [&catalog::TALKER, &catalog::CODEC, &catalog::RUNTIME]
        .into_iter()
        .map(|a| {
            std::fs::metadata(root.join(format!("{}.part", a.file)))
                .map(|m| m.len())
                .unwrap_or(0)
        })
        .sum()
}

/// Download, verify and unpack everything that is missing.
pub async fn install(
    root: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(&str, u64, u64),
) -> Result<(), String> {
    for dir in ["models", catalog::RUNTIME_DIR] {
        std::fs::create_dir_all(root.join(dir)).map_err(|e| e.to_string())?;
    }
    let need = missing_bytes(root);
    if fs2::available_space(root).map_err(|e| e.to_string())? < need + 200_000_000 {
        return Err(crate::preferences::message(
            &format!(
                "Not enough disk space. Qwen3-TTS needs {} MB free.",
                (need + 200_000_000).div_ceil(1_000_000)
            ),
            &format!(
                "Недостаточно места. Для Qwen3-TTS нужно {} МБ свободного места.",
                (need + 200_000_000).div_ceil(1_000_000)
            ),
        ));
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let runtime_ready = {
        let root = root.to_path_buf();
        tokio::task::spawn_blocking(move || {
            catalog::DLLS
                .iter()
                .all(|a| download::verify(&root.join(a.file), a).is_ok())
        })
        .await
        .map_err(|e| e.to_string())?
    };
    let artifacts: Vec<&Artifact> = [&catalog::TALKER, &catalog::CODEC]
        .into_iter()
        .chain((!runtime_ready).then_some(&catalog::RUNTIME))
        .collect();
    let total: u64 = artifacts.iter().map(|a| a.bytes).sum();
    let mut complete = 0;
    for artifact in artifacts {
        download::fetch(&client, artifact, root, cancel, |n| {
            progress("downloading", complete + n, total)
        })
        .await?;
        complete += artifact.bytes;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Загрузка отменена. Её можно продолжить.".into());
    }
    progress("verifying", total, total);
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if !runtime_ready {
            unpack(&root)?;
        }
        std::fs::write(root.join("Qwen3-TTS-LICENSE.txt"), LICENSE).map_err(|e| e.to_string())?;
        verify(&root)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Unpack only the pinned DLLs from the verified archive, each checked by
/// size before writing and by hash after; nothing else is extracted.
fn unpack(root: &Path) -> Result<(), String> {
    download::verify(&root.join(catalog::RUNTIME.file), &catalog::RUNTIME)?;
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(root.join(catalog::RUNTIME.file)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let staging = root.join("runtime-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    for artifact in catalog::DLLS {
        let name = Path::new(artifact.file)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid runtime file name")?;
        let mut entry = archive.by_name(name).map_err(|e| e.to_string())?;
        if entry.size() != artifact.bytes || entry.is_symlink() || entry.is_dir() {
            return Err("Invalid Qwen3-TTS runtime archive.".into());
        }
        let target = staging.join(name);
        let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        drop(file);
        download::verify(&target, artifact)?;
    }
    let runtime = root.join(catalog::RUNTIME_DIR);
    let _ = std::fs::remove_dir_all(&runtime);
    std::fs::rename(&staging, &runtime).map_err(|e| e.to_string())
}

/// Remove every Qwen file: models, runtime, archive and partial downloads.
pub fn remove(root: &Path) -> Result<(), String> {
    for artifact in [&catalog::TALKER, &catalog::CODEC, &catalog::RUNTIME] {
        for path in [
            root.join(artifact.file),
            root.join(format!("{}.part", artifact.file)),
        ] {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    for dir in [catalog::RUNTIME_DIR, "runtime-staging"] {
        match std::fs::remove_dir_all(root.join(dir)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let _ = std::fs::remove_file(root.join("device-profile.txt"));
    Ok(())
}

/// `glagol.exe --qwen-worker <root> <dir> <parent pid> <language> <profile>`.
pub fn child_entry() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--qwen-worker") {
        return None;
    }
    if args.len() != 7 {
        return Some(2);
    }
    let (Some(parent), Some(language), Some(profile)) = (
        args[4].to_str().and_then(|a| a.parse::<u32>().ok()),
        args[5].to_str(),
        args[6].to_str(),
    ) else {
        return Some(2);
    };
    Some(
        match worker::child_main(
            Path::new(&args[2]),
            Path::new(&args[3]),
            parent,
            language,
            profile,
        ) {
            Ok(()) => 0,
            Err(_) => 1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_installed_follow_the_pinned_sizes() {
        let root = std::env::temp_dir().join(format!("qwen-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("models")).unwrap();
        assert!(!installed(&root));
        let all = catalog::TALKER.bytes + catalog::CODEC.bytes + catalog::RUNTIME.bytes;
        assert_eq!(missing_bytes(&root), all);
        std::fs::write(
            root.join(format!("{}.part", catalog::CODEC.file)),
            vec![0u8; 1000],
        )
        .unwrap();
        assert_eq!(missing_bytes(&root), all - 1000);
        assert_eq!(partial_bytes(&root), 1000);
        remove(&root).unwrap();
        assert_eq!(partial_bytes(&root), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_arguments_are_strict() {
        // Not a worker invocation: the test binary's own arguments.
        assert!(child_entry().is_none());
    }
}
