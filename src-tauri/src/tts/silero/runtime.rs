use super::catalog;
use crate::stt::local::download;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path},
    sync::atomic::{AtomicBool, Ordering},
};

fn checked(root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    if name.contains(['\\', ':'])
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Недопустимый путь в архиве TTS.".into());
    }
    Ok(root.join(name))
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Операция отменена.".into())
    } else {
        Ok(())
    }
}
pub fn verify(root: &Path, cancel: &AtomicBool) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Inventory {
        files: BTreeMap<String, String>,
    }
    let inventory: Inventory =
        serde_json::from_str(include_str!("runtime-files.json")).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 65536];
    for (name, expected) in inventory.files {
        cancelled(cancel)?;
        let mut file = fs::File::open(checked(root, &name)?).map_err(|_| {
            "Движок отсутствует или повреждён. Нажмите «Восстановить» в настройках озвучки."
                .to_string()
        })?;
        let mut hash = Sha256::new();
        loop {
            cancelled(cancel)?;
            let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        if format!("{:x}", hash.finalize()) != expected {
            return Err("Файлы движка повреждены. Восстановите озвучку в настройках.".into());
        }
    }
    Ok(())
}
pub fn assemble(root: &Path, cancel: &AtomicBool) -> Result<(), String> {
    for artifact in catalog::ARTIFACTS {
        cancelled(cancel)?;
        download::verify(&root.join(artifact.file), artifact)?;
    }
    let staging = root.join(format!("runtime-staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        let site = staging.join("Lib/site-packages");
        for artifact in catalog::ARTIFACTS {
            cancelled(cancel)?;
            let file = fs::File::open(root.join(artifact.file)).map_err(|e| e.to_string())?;
            if artifact.file.ends_with(".tar.gz") {
                let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
                for entry in tar.entries().map_err(|e| e.to_string())? {
                    let mut entry = entry.map_err(|e| e.to_string())?;
                    let name = entry
                        .path()
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .into_owned();
                    let output = match name.as_str() {
                        "docopt-0.6.2/docopt.py" => Some("docopt.py"),
                        "docopt-0.6.2/LICENSE-MIT" => Some("docopt-LICENSE-MIT.txt"),
                        _ => None,
                    };
                    if let Some(name) = output {
                        if !entry.header().entry_type().is_file() {
                            return Err("Недопустимый тип файла TTS.".into());
                        }
                        let mut target =
                            fs::File::create(site.join(name)).map_err(|e| e.to_string())?;
                        std::io::copy(&mut entry, &mut target).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                let destination = if artifact.file.ends_with(".whl") {
                    &site
                } else {
                    &staging
                };
                let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                for i in 0..zip.len() {
                    cancelled(cancel)?;
                    let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
                    let name = entry.name().trim_end_matches('/');
                    let output = checked(destination, name)?;
                    if entry.is_symlink() {
                        return Err("Ссылка в архиве TTS недопустима.".into());
                    }
                    if entry.is_dir() {
                        fs::create_dir_all(&output).map_err(|e| e.to_string())?;
                    } else {
                        fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
                        let mut target = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(output)
                            .map_err(|e| e.to_string())?;
                        std::io::copy(&mut entry, &mut target).map_err(|e| e.to_string())?;
                    }
                }
            }
        }
        fs::write(
            staging.join("python311._pth"),
            b"python311.zip\r\n.\r\nLib/site-packages\r\n",
        )
        .map_err(|e| e.to_string())?;
        verify(&staging, cancel)?;
        let destination = root.join("runtime");
        let previous = root.join(format!("runtime-previous-{}", uuid::Uuid::new_v4()));
        if destination.exists() {
            fs::rename(&destination, &previous).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&staging, &destination) {
            let _ = fs::rename(&previous, &destination);
            return Err(error.to_string());
        }
        if previous.exists() {
            let _ = fs::remove_dir_all(previous);
        }
        Ok(())
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(staging);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_unsafe_archive_paths() {
        for name in [
            "../escape",
            "/absolute",
            "C:/windows",
            "file:stream",
            "dir\\file",
        ] {
            assert!(checked(Path::new("runtime"), name).is_err());
        }
        assert_eq!(
            checked(Path::new("runtime"), "Lib/file.py").unwrap(),
            Path::new("runtime/Lib/file.py")
        );
    }
}
