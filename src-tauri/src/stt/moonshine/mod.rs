pub mod catalog;
mod engine;
pub mod worker;
use crate::stt::local::{catalog::Artifact, download};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::Emitter;

pub struct Moonshine {
    pub root: PathBuf,
    pub operation: Arc<tokio::sync::Mutex<()>>,
    worker: tokio::sync::Mutex<Option<worker::Worker>>,
}

pub struct Provider {
    pub models: Arc<Moonshine>,
    stream: std::sync::Mutex<Option<worker::Streaming>>,
}
impl Provider {
    pub fn new(models: Arc<Moonshine>) -> Self {
        Self {
            models,
            stream: std::sync::Mutex::new(None),
        }
    }
    pub async fn begin(&self) -> Result<worker::Input, String> {
        let stream = self.models.begin().await?;
        let input = stream.input.clone();
        *self.stream.lock().map_err(|e| e.to_string())? = Some(stream);
        Ok(input)
    }
}
impl crate::stt::SttProvider for Provider {
    async fn transcribe(
        &self,
        _wav: Vec<u8>,
        _lang: Option<&str>,
    ) -> Result<crate::stt::Transcript, crate::stt::SttError> {
        let receiver = self
            .stream
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.result.lock().unwrap().take())
            .ok_or_else(|| crate::stt::SttError::Api("No active Moonshine recording.".into()))?;
        let text = receiver
            .await
            .map_err(|_| crate::stt::SttError::Api("The recognition worker exited.".into()))?
            .map_err(crate::stt::SttError::Api)?;
        Ok(crate::stt::Transcript { text })
    }
    async fn list_models(&self) -> Result<Vec<String>, crate::stt::SttError> {
        Ok(vec![catalog::ID.into()])
    }
}
impl Moonshine {
    pub fn new(root: PathBuf, operation: Arc<tokio::sync::Mutex<()>>) -> Self {
        Self {
            root,
            operation,
            worker: tokio::sync::Mutex::new(None),
        }
    }
    pub async fn close(&self) {
        if let Some(mut worker) = self.worker.lock().await.take() {
            worker.shutdown().await;
        }
    }
    pub async fn begin(self: &Arc<Self>) -> Result<worker::Streaming, String> {
        let operation = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| "Wait for the current model operation to finish.")?;
        let failed = Arc::new(AtomicBool::new(false));
        let mut worker = match self.worker.lock().await.take() {
            Some(worker) => worker,
            None => worker::Worker::start(&self.root, &failed).await?,
        };
        worker.request(worker::Request::Begin, &failed).await?;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(32);
        let (send_result, result) = tokio::sync::oneshot::channel();
        let this = self.clone();
        let flag = failed.clone();
        tokio::spawn(async move {
            let _operation = operation;
            let result = async {
                loop {
                    let packet = tokio::select! {
                        packet = receiver.recv() => packet.ok_or("Recognition cancelled.")?,
                        _ = async { while !flag.load(Ordering::Relaxed) { tokio::time::sleep(Duration::from_millis(50)).await; } } => return Err("Recognition stopped after an engine or audio queue error.".into()),
                    };
                    match packet {
                        worker::Packet::Audio(samples) => { worker.request(worker::Request::Audio { samples }, &flag).await?; }
                        worker::Packet::Finish => return worker.request(worker::Request::Finish, &flag).await,
                    }
                }
            }.await;
            if result.is_ok() {
                *this.worker.lock().await = Some(worker);
            } else {
                flag.store(true, Ordering::Relaxed);
                worker.shutdown().await;
            }
            let _ = send_result.send(result);
        });
        Ok(worker::Streaming {
            input: worker::Input { sender, failed },
            result: std::sync::Mutex::new(Some(result)),
        })
    }
    pub fn idle_task(self: &Arc<Self>) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let mut slot = this.worker.lock().await;
                if slot
                    .as_ref()
                    .is_some_and(|w| w.last_used.elapsed() >= Duration::from_secs(15 * 60))
                {
                    if let Some(mut worker) = slot.take() {
                        worker.shutdown().await;
                    }
                }
            }
        });
    }
}

pub fn verify(root: &Path) -> Result<(), String> {
    for artifact in catalog::FILES.iter().chain(catalog::DLLS) {
        download::verify(&root.join(artifact.file), artifact)?;
    }
    Ok(())
}
pub fn installed(root: &Path) -> bool {
    catalog::FILES
        .iter()
        .chain(catalog::DLLS)
        .all(|a| std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes))
}
pub fn partial_bytes(root: &Path) -> u64 {
    catalog::FILES
        .iter()
        .chain([&catalog::RUNTIME])
        .map(|a| {
            std::fs::metadata(root.join("staging").join(format!("{}.part", a.file)))
                .map(|m| m.len().min(a.bytes))
                .unwrap_or(0)
        })
        .sum()
}
pub fn download_bytes(root: &Path) -> u64 {
    let models = catalog::FILES
        .iter()
        .filter(|a| !std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes))
        .map(|a| a.bytes)
        .sum::<u64>();
    models
        + if catalog::DLLS
            .iter()
            .all(|a| std::fs::metadata(root.join(a.file)).is_ok_and(|m| m.len() == a.bytes))
        {
            0
        } else {
            catalog::RUNTIME.bytes
        }
}

pub async fn install(
    app: &tauri::AppHandle,
    root: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(&str, u64, u64),
) -> Result<(), String> {
    std::fs::create_dir_all(root.join("staging/stt")).map_err(|e| e.to_string())?;
    let staging = root.join("staging");
    let check_root = root.to_owned();
    let reuse_runtime = tokio::task::spawn_blocking(move || runtime_valid(&check_root))
        .await
        .map_err(|e| e.to_string())?;
    let total = catalog::FILES.iter().map(|a| a.bytes).sum::<u64>()
        + if reuse_runtime {
            0
        } else {
            catalog::RUNTIME.bytes
        };
    if fs2::available_space(root).map_err(|e| e.to_string())? < total + 30_000_000 {
        return Err("Not enough disk space for Moonshine. Free 200 MB and try again.".into());
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let mut complete = 0;
    for artifact in catalog::FILES.iter().chain([&catalog::RUNTIME]) {
        if reuse_runtime && artifact.file == catalog::RUNTIME.file {
            continue;
        }
        // Reuse verified installed files, retaining resumable staging downloads.
        let final_path = root.join(artifact.file);
        let data = *artifact;
        let valid =
            tokio::task::spawn_blocking(move || download::verify(&final_path, &data).is_ok())
                .await
                .map_err(|e| e.to_string())?;
        if !valid {
            download::fetch(&client, artifact, &staging, cancel, |n| {
                progress("downloading", complete + n, total)
            })
            .await?;
        }
        complete += artifact.bytes;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Download cancelled.".into());
    }
    progress("verifying", total, total);
    let root = root.to_owned();
    tokio::task::spawn_blocking(move || commit(&root))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.emit("local-models-changed", ());
    Ok(())
}

fn commit(root: &Path) -> Result<(), String> {
    let staging = root.join("staging");
    if !runtime_valid(root) {
        let runtime = if staging.join(catalog::RUNTIME.file).is_file() {
            staging.join(catalog::RUNTIME.file)
        } else {
            root.join(catalog::RUNTIME.file)
        };
        download::verify(&runtime, &catalog::RUNTIME)?;
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(runtime).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(staging.join("native")).map_err(|e| e.to_string())?;
        for (artifact, entry) in catalog::DLLS.iter().zip(catalog::ENTRIES) {
            let mut entry = archive.by_name(entry).map_err(|e| e.to_string())?;
            if entry.size() != artifact.bytes || entry.is_symlink() {
                return Err("Invalid Moonshine runtime archive.".into());
            }
            let mut file =
                std::fs::File::create(staging.join(artifact.file)).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            download::verify(&staging.join(artifact.file), artifact)?;
        }
    }
    // Verify the entire set before exposing any file. Startup rechecks every hash
    // so interruption during the rename phase can never execute a partial set.
    for artifact in catalog::FILES.iter().chain(catalog::DLLS) {
        let candidate = if staging.join(artifact.file).is_file() {
            staging.join(artifact.file)
        } else {
            root.join(artifact.file)
        };
        download::verify(&candidate, artifact)?;
    }
    for artifact in catalog::FILES
        .iter()
        .chain(catalog::DLLS)
        .chain([&catalog::RUNTIME])
    {
        let candidate = staging.join(artifact.file);
        if candidate.is_file() {
            let target = root.join(artifact.file);
            std::fs::create_dir_all(target.parent().ok_or("Invalid artifact path")?)
                .map_err(|e| e.to_string())?;
            std::fs::rename(candidate, target).map_err(|e| e.to_string())?;
        }
    }
    std::fs::create_dir_all(root.join("licenses")).map_err(|e| e.to_string())?;
    for (name, text) in [
        (
            "Moonshine-LICENSE.txt",
            include_str!("../../../../docs/third-party/Moonshine-LICENSE.txt"),
        ),
        (
            "ONNX-Runtime-LICENSE.txt",
            include_str!("../../../../docs/third-party/ONNX-Runtime-LICENSE.txt"),
        ),
        (
            "ONNX-Runtime-ThirdPartyNotices.txt",
            include_str!("../../../../docs/third-party/ONNX-Runtime-ThirdPartyNotices.txt"),
        ),
    ] {
        std::fs::write(root.join("licenses").join(name), text).map_err(|e| e.to_string())?;
    }
    verify(root)
}

fn runtime_valid(root: &Path) -> bool {
    catalog::DLLS
        .iter()
        .all(|artifact| download::verify(&root.join(artifact.file), artifact).is_ok())
}

pub fn remove(root: &Path) -> Result<(), String> {
    // Only fixed model paths; keep the separately versioned runtime.
    for Artifact { file, .. } in catalog::FILES {
        for candidate in [
            root.join(file),
            root.join("staging").join(file),
            root.join("staging").join(format!("{file}.part")),
        ] {
            if candidate.is_file() {
                std::fs::remove_file(candidate).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

pub fn child_entry() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|a| a != "--moonshine-worker") {
        return None;
    }
    if args.len() != 4 {
        return Some(2);
    }
    let parent = args[3].to_str().and_then(|a| a.parse::<u32>().ok());
    Some(match parent {
        Some(parent) => {
            if worker::child_main(Path::new(&args[2]), parent).is_ok() {
                0
            } else {
                1
            }
        }
        None => 2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned artifacts in GLAGOL_MOONSHINE_ROOT"]
    fn native_moonshine_reuses_dlls_without_wheel_and_keeps_them_on_removal() {
        let fixture = PathBuf::from(std::env::var("GLAGOL_MOONSHINE_ROOT").unwrap());
        let root = std::env::temp_dir().join(format!("moonshine-reuse-{}", uuid::Uuid::new_v4()));
        for artifact in catalog::FILES.iter().chain(catalog::DLLS) {
            let target = root.join(artifact.file);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::hard_link(fixture.join(artifact.file), target).unwrap();
        }
        assert!(!root.join(catalog::RUNTIME.file).exists());
        assert_eq!(download_bytes(&root), 0);
        commit(&root).unwrap();
        remove(&root).unwrap();
        assert!(!installed(&root));
        assert!(runtime_valid(&root));
        assert_eq!(
            download_bytes(&root),
            catalog::FILES.iter().map(|a| a.bytes).sum::<u64>()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_or_corrupt_set_cannot_be_committed() {
        let root =
            std::env::temp_dir().join(format!("moonshine-incomplete-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("staging")).unwrap();
        std::fs::write(
            root.join("staging").join(catalog::RUNTIME.file),
            b"damaged archive",
        )
        .unwrap();
        assert!(!installed(&root));
        assert!(verify(&root).is_err());
        assert!(commit(&root).is_err());
        assert!(!root.join("native").exists());
        assert!(!root.join("stt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
