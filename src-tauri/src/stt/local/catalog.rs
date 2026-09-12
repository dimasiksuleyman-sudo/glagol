//! Pinned, data-only model catalog. URLs and hashes never come from the webview.
use serde::Serialize;

pub const RUNTIME_VERSION: &str = "0.2.3";
pub const RUNTIME_DIR: &str = "transcribe-native-windows-x86_64-cpu-vulkan";
pub const RUNTIME: Artifact = Artifact {
    file: "runtime-0.2.3.tar.gz",
    url: "https://github.com/handy-computer/transcribe.cpp/releases/download/v0.2.3/transcribe-native-0.2.3-windows-x86_64-cpu-vulkan.tar.gz",
    bytes: 20_077_848,
    sha256: "dac5b6038aaf8777cab541b0f854a79e34f13e5892266229f64c61d34a879e49",
};

#[derive(Clone, Copy, Serialize)]
pub struct Artifact {
    pub file: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Serialize)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub artifact: Artifact,
}

pub const MODELS: [Model; 2] = [
    Model {
        id: "gigaam-v3-e2e-ctc",
        name: "GigaAM v3 · CTC",
        description: "Русская речь с пунктуацией. Компактный вариант для повседневной диктовки.",
        artifact: Artifact {
            file: "gigaam-v3-e2e-ctc-Q8_0.gguf",
            url: "https://huggingface.co/handy-computer/gigaam-v3-e2e-ctc-gguf/resolve/075dff81f843cf23d22b4ce943ffdc4dd8650cd7/gigaam-v3-e2e-ctc-Q8_0.gguf",
            bytes: 272_151_136,
            sha256: "9ccce4750dc813a493d96ca15ee251712bedec15ac9a02fa3d2bd732f08ae5eb",
        },
    },
    Model {
        id: "gigaam-v3-e2e-rnnt",
        name: "GigaAM v3 · RNNT",
        description: "Русская речь с пунктуацией. Альтернативный вариант — сравните на своих фразах.",
        artifact: Artifact {
            file: "gigaam-v3-e2e-rnnt-Q8_0.gguf",
            url: "https://huggingface.co/handy-computer/gigaam-v3-e2e-rnnt-gguf/resolve/f719d70812344f4d0fb8c11c0887b190501a7465/gigaam-v3-e2e-rnnt-Q8_0.gguf",
            bytes: 273_724_832,
            sha256: "78d63b47723b7f8d78c6113a6ef983b5a86e2a86f6c273e1f5cb6967b1c4467a",
        },
    },
];

pub fn model(id: &str) -> Result<&'static Model, String> {
    MODELS
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| "Неизвестная локальная модель.".into())
}
