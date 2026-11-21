//! Pinned Qwen3-TTS files. The runtime archive is built by
//! `.github/workflows/qwen-runtime.yml` from qwentts.cpp 2edb35f8 (MIT) and
//! published as the pre-release `qwen-runtime-2edb35f`; the GGUF models are
//! Apache 2.0 (HF Serveurperso/Qwen3-TTS-GGUF, revision b7ee2e8c).
use crate::stt::local::catalog::Artifact;

/// Folder of the unpacked runtime DLLs (the worker's current directory).
pub const RUNTIME_DIR: &str = "runtime";

pub const RUNTIME: Artifact = Artifact {
    file: "qwen-runtime-windows-x64-2edb35f.zip",
    url: "https://github.com/dimasiksuleyman-sudo/glagol/releases/download/qwen-runtime-2edb35f/qwen-runtime-windows-x64-2edb35f.zip",
    bytes: 16174644,
    sha256: "216e669f4ba77fcba2007426ceda6b2dab631217767a40db697638c3c023d1cc",
};

/// Talker (Qwen3 LM + code predictor), 0.6B CustomVoice, Q8_0.
pub const TALKER: Artifact = Artifact {
    file: "models/qwen-talker-0.6b-customvoice-Q8_0.gguf",
    url: "https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF/resolve/b7ee2e8c7459c3bea99da23e3d178125a7d1713c/qwen-talker-0.6b-customvoice-Q8_0.gguf",
    bytes: 968588544,
    sha256: "4eb38675c736ed6ac72012846ac8d6ef80e5af8bc05726870f0b3a6569588519",
};

/// 12 Hz audio codec (tokenizer), Q8_0.
pub const CODEC: Artifact = Artifact {
    file: "models/qwen-tokenizer-12hz-Q8_0.gguf",
    url: "https://huggingface.co/Serveurperso/Qwen3-TTS-GGUF/resolve/b7ee2e8c7459c3bea99da23e3d178125a7d1713c/qwen-tokenizer-12hz-Q8_0.gguf",
    bytes: 291150624,
    sha256: "1883beeed99348fc35e23dd225e9082f93f6f8c109330a33d935baa8acdbfd94",
};

/// Every DLL unpacked from [`RUNTIME`]; nothing else is extracted.
pub const DLLS: &[Artifact] = &[
    Artifact {
        file: "runtime/ggml-base.dll",
        url: "",
        bytes: 730624,
        sha256: "542acb2f905e3660906063876ac425754af8cae02c41544be686fe2e319e11f6",
    },
    Artifact {
        file: "runtime/ggml-cpu-alderlake.dll",
        url: "",
        bytes: 1021952,
        sha256: "6b69d3b539f6fd7f392023187e8bbb1b751b852d6cea522572d080012c5abcd9",
    },
    Artifact {
        file: "runtime/ggml-cpu-cannonlake.dll",
        url: "",
        bytes: 1065984,
        sha256: "4cfffe30842acd2730968f9ada50ba1cd3694b009113a1a63a52d31e848c4cb1",
    },
    Artifact {
        file: "runtime/ggml-cpu-cascadelake.dll",
        url: "",
        bytes: 1070592,
        sha256: "2a0eda3b902584df4847c1d2a340e71567c69fb120808cdf26dda5c569a0f497",
    },
    Artifact {
        file: "runtime/ggml-cpu-haswell.dll",
        url: "",
        bytes: 1023488,
        sha256: "685a22cf8061030b98b8c27691a60efb70ae8cad9bef7f1c621910c4fdd8daa8",
    },
    Artifact {
        file: "runtime/ggml-cpu-icelake.dll",
        url: "",
        bytes: 1070592,
        sha256: "4bed4f451b8344bc0301758a33372e0d60ad92b852e18de89914d333b77b33ca",
    },
    Artifact {
        file: "runtime/ggml-cpu-sandybridge.dll",
        url: "",
        bytes: 1053184,
        sha256: "95fa6c0b08284e5c4c8fc493df558e91d0aeff1f7f9955bdc5473f52462b196e",
    },
    Artifact {
        file: "runtime/ggml-cpu-skylakex.dll",
        url: "",
        bytes: 1065984,
        sha256: "dc761c9e8e2a7ff06348a447b36ac5abbc70018d20ffee8bdab5f36b3df6475f",
    },
    Artifact {
        file: "runtime/ggml-cpu-sse42.dll",
        url: "",
        bytes: 855040,
        sha256: "12b2e5942ccb51a823c720a511c9b1887e4f78c333042d9c2381d8ae8ed24df7",
    },
    Artifact {
        file: "runtime/ggml-cpu-x64.dll",
        url: "",
        bytes: 860160,
        sha256: "a3e7746ddfeaf0c3b35f4264044b5add73a61fe9e1684b41c32568e08abbc49d",
    },
    Artifact {
        file: "runtime/ggml-vulkan.dll",
        url: "",
        bytes: 40564224,
        sha256: "2cad1afa529fc3ace731e370aa28656934c1009d47fc6d086bd5ed6ffb760d47",
    },
    Artifact {
        file: "runtime/ggml.dll",
        url: "",
        bytes: 62464,
        sha256: "1ce6a3b5291e5e6d8509c310f85b5b6564d0a5f35bae0643e8f5d27a978d2ae7",
    },
    Artifact {
        file: "runtime/qwen.dll",
        url: "",
        bytes: 310784,
        sha256: "bc72b230dfc5875aa065e4a4a8d19819ce2c607ac47e7e792d4e66df847e3ba7",
    },
];
