//! Resumable, bounded downloads; only verified files get their final name.
use super::catalog::Artifact;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

pub fn verify(path: &Path, artifact: &Artifact) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != artifact.bytes {
        return Err("Размер загруженного файла не совпадает. Повторите загрузку.".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err("Проверка целостности не пройдена. Повторите загрузку.".into());
    }
    Ok(())
}

async fn verify_async(path: &Path, artifact: &Artifact) -> Result<(), String> {
    let path = path.to_path_buf();
    let artifact = *artifact;
    tokio::task::spawn_blocking(move || verify(&path, &artifact))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn fetch(
    client: &reqwest::Client,
    artifact: &Artifact,
    root: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<(), String> {
    let final_path = root.join(artifact.file);
    if final_path.exists() && verify_async(&final_path, artifact).await.is_ok() {
        progress(artifact.bytes);
        return Ok(());
    }
    let partial = root.join(format!("{}.part", artifact.file));
    let mut offset = tokio::fs::metadata(&partial)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    if offset > artifact.bytes {
        tokio::fs::remove_file(&partial)
            .await
            .map_err(|e| e.to_string())?;
        offset = 0;
    }
    if offset < artifact.bytes {
        if cancel.load(Ordering::Relaxed) {
            return Err("Загрузка отменена. Её можно продолжить.".into());
        }
        let mut request = client
            .get(artifact.url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let mut response = request.send().await.map_err(|_| {
            "Не удалось скачать файл. Проверьте интернет и повторите загрузку.".to_string()
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "Сервер загрузки вернул HTTP {}. Повторите позже.",
                status.as_u16()
            ));
        }
        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            let expected = format!("bytes {offset}-{}/{}", artifact.bytes - 1, artifact.bytes);
            if response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|h| h.to_str().ok())
                != Some(expected.as_str())
            {
                return Err("Сервер вернул неверный диапазон файла.".into());
            }
        } else {
            offset = 0;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(offset > 0)
            .truncate(offset == 0)
            .open(&partial)
            .await
            .map_err(|e| format!("Не удалось записать модель: {e}"))?;
        progress(offset);
        loop {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await.map_err(|e| e.to_string())?;
                return Err("Загрузка отменена. Её можно продолжить.".into());
            }
            // Poll cancellation even if the server stalls between packets.
            let next = response.chunk();
            tokio::pin!(next);
            let chunk = loop {
                tokio::select! {
                    result = &mut next => break result.map_err(|_| "Загрузка прервана. Нажмите «Продолжить».".to_string())?,
                    _ = tokio::time::sleep(Duration::from_millis(150)) => {
                        if cancel.load(Ordering::Relaxed) { return Err("Загрузка отменена. Её можно продолжить.".into()); }
                    }
                }
            };
            let Some(chunk) = chunk else {
                break;
            };
            offset += chunk.len() as u64;
            if offset > artifact.bytes {
                return Err("Сервер прислал файл неверного размера.".into());
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Не удалось записать модель: {e}"))?;
            progress(offset);
        }
        file.sync_all().await.map_err(|e| e.to_string())?;
    }
    if let Err(error) = verify_async(&partial, artifact).await {
        // Incomplete downloads are resumable; complete but corrupt ones must restart.
        if offset == artifact.bytes {
            let _ = tokio::fs::remove_file(&partial).await;
        }
        return Err(error);
    }
    if final_path.exists() {
        tokio::fs::remove_file(&final_path)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::rename(partial, final_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("glagol-download-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn artifact(url: String) -> Artifact {
        Artifact {
            file: "model.bin",
            url: Box::leak(url.into_boxed_str()),
            bytes: 6,
            sha256: "bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721",
        }
    }
    #[tokio::test]
    async fn resumes_valid_range_and_promotes_only_verified_file() {
        let mut server = mockito::Server::new_async().await;
        let a = artifact(server.url());
        let dir = Temp::new();
        std::fs::write(dir.0.join("model.bin.part"), b"abc").unwrap();
        let mock = server
            .mock("GET", "/")
            .match_header("range", "bytes=3-")
            .with_status(206)
            .with_header("content-range", "bytes 3-5/6")
            .with_body("def")
            .create_async()
            .await;
        fetch(
            &reqwest::Client::new(),
            &a,
            &dir.0,
            &AtomicBool::new(false),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(dir.0.join(a.file)).unwrap(), b"abcdef");
        assert!(!dir.0.join("model.bin.part").exists());
        mock.assert_async().await;
    }
    #[tokio::test]
    async fn server_ignoring_range_restarts_without_appending() {
        let mut server = mockito::Server::new_async().await;
        let a = artifact(server.url());
        let dir = Temp::new();
        std::fs::write(dir.0.join("model.bin.part"), b"abc").unwrap();
        let _mock = server
            .mock("GET", "/")
            .with_body("abcdef")
            .create_async()
            .await;
        fetch(
            &reqwest::Client::new(),
            &a,
            &dir.0,
            &AtomicBool::new(false),
            |_| {},
        )
        .await
        .unwrap();
        verify(&dir.0.join(a.file), &a).unwrap();
    }
    #[tokio::test]
    async fn corrupt_or_oversized_download_never_becomes_installed() {
        for body in ["abcdeg", "abcdefghi"] {
            let mut server = mockito::Server::new_async().await;
            let a = artifact(server.url());
            let dir = Temp::new();
            let _mock = server.mock("GET", "/").with_body(body).create_async().await;
            assert!(fetch(
                &reqwest::Client::new(),
                &a,
                &dir.0,
                &AtomicBool::new(false),
                |_| {}
            )
            .await
            .is_err());
            assert!(!dir.0.join(a.file).exists());
        }
    }
    #[tokio::test]
    async fn wrong_range_is_rejected_without_modifying_partial() {
        let mut server = mockito::Server::new_async().await;
        let a = artifact(server.url());
        let dir = Temp::new();
        std::fs::write(dir.0.join("model.bin.part"), b"abc").unwrap();
        let _mock = server
            .mock("GET", "/")
            .with_status(206)
            .with_header("content-range", "bytes 0-2/6")
            .with_body("def")
            .create_async()
            .await;
        assert!(fetch(
            &reqwest::Client::new(),
            &a,
            &dir.0,
            &AtomicBool::new(false),
            |_| {}
        )
        .await
        .is_err());
        assert_eq!(std::fs::read(dir.0.join("model.bin.part")).unwrap(), b"abc");
    }
    #[tokio::test]
    async fn cancellation_preserves_partial_for_next_attempt() {
        let dir = Temp::new();
        let a = artifact("http://127.0.0.1:1".into());
        std::fs::write(dir.0.join("model.bin.part"), b"abc").unwrap();
        assert!(fetch(
            &reqwest::Client::new(),
            &a,
            &dir.0,
            &AtomicBool::new(true),
            |_| {}
        )
        .await
        .unwrap_err()
        .contains("отменена"));
        assert_eq!(std::fs::read(dir.0.join("model.bin.part")).unwrap(), b"abc");
    }
}
