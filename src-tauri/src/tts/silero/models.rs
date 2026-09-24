//! Language-specific models share one verified Python/PyTorch installation.
use super::{catalog, Silero};
use crate::{preferences::Language, stt::local::catalog::Artifact};
use std::{path::PathBuf, sync::Arc};

pub const EN_VOICES: &[&str] = &["en_0", "en_1", "en_2", "en_3"];

/// Pinned model identity; never accepts an arbitrary PyTorch package.
pub struct Model {
    pub id: &'static str,
    pub language: Language,
    pub artifact: &'static Artifact,
    pub voices: &'static [&'static str],
    pub provider: &'static str,
    pub acknowledgement: &'static str,
    pub verification: &'static str,
}
pub const RU: Model = Model {
    id: super::MODEL_ID,
    language: Language::Ru,
    artifact: &catalog::MODEL,
    voices: super::VOICES,
    provider: "silero",
    acknowledgement: "acknowledgement.json",
    verification: super::VERIFICATION_STAMP,
};
pub const EN: Model = Model {
    id: "silero-v3-en",
    language: Language::En,
    artifact: &catalog::EN_MODEL,
    voices: EN_VOICES,
    provider: "silero-en",
    acknowledgement: "acknowledgement-en.json",
    verification: "verification-en.json",
};

/// One operation lock prevents shared runtime replacement during inference.
pub struct Models {
    pub ru: Arc<Silero>,
    pub en: Arc<Silero>,
}
impl Models {
    /// Reuse the existing RU manager and its root and lock, preserving old installs.
    pub fn new(ru: Arc<Silero>) -> Self {
        let en = Arc::new(Silero::for_model(
            ru.root.clone(),
            &EN,
            ru.operation.clone(),
        ));
        Self { ru, en }
    }
    /// Provider IDs are validated before accessing any files.
    pub fn select(&self, provider: Option<&str>) -> Result<Arc<Silero>, String> {
        match provider.unwrap_or("silero") {
            "silero" => Ok(self.ru.clone()),
            "silero-en" => Ok(self.en.clone()),
            _ => Err(crate::preferences::message(
                "Unknown speech provider.",
                "Неизвестный провайдер озвучки.",
            )),
        }
    }
    /// Stop both owned processes before a shared runtime installation or repair.
    pub async fn close_workers(&self) {
        self.ru.close_worker().await;
        self.en.close_worker().await;
    }
    /// Remove only this language's model, partial download and metadata.
    pub fn remove_model_files(&self, model: &Silero) -> Result<(), String> {
        for file in [
            model.model.artifact.file,
            model.model.acknowledgement,
            model.model.verification,
        ] {
            remove_if_present(model.root.join(file))?;
        }
        remove_if_present(
            model
                .root
                .join(format!("{}.part", model.model.artifact.file)),
        )?;
        Ok(())
    }
}
fn remove_if_present(path: PathBuf) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn languages_share_runtime_and_lock_but_not_consent_or_receipts() {
        let root = std::env::temp_dir().join(format!("silero-language-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        for (file, data) in [
            ("runtime/python.exe", b"python".as_slice()),
            ("runtime/python311.dll", b"dll"),
            (catalog::MODEL.file, b"ru"),
            (catalog::EN_MODEL.file, b"en"),
        ] {
            std::fs::write(root.join(file), data).unwrap();
        }
        let models = Models::new(Arc::new(Silero::new(root.clone())));
        models
            .ru
            .acknowledge(true, super::super::LICENSE_HASH)
            .unwrap();
        assert!(!models.en.accepted());
        models.ru.record_verification().unwrap();
        assert!(models.ru.recent_verification_valid());
        assert!(!models.en.recent_verification_valid());
        models.en.record_verification().unwrap();
        assert!(models.en.recent_verification_valid());
        let guard = models.ru.operation.lock().await;
        assert!(models.en.operation.try_lock().is_err());
        drop(guard);
        models.remove_model_files(&models.en).unwrap();
        assert!(root.join("runtime/python.exe").exists());
        assert!(root.join(catalog::MODEL.file).exists());
        assert!(models.ru.accepted());
        assert!(models.ru.recent_verification_valid());
        assert!(!root.join(catalog::EN_MODEL.file).exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
