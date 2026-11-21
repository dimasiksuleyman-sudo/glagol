//! The pinned qwentts.cpp ABI (`qt_*`, ABI 6) is used only inside the
//! disposable worker process, after every runtime and model file is verified.
use libloading::Library;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
};

/// `struct qt_init_params` of the pinned `qwen.h` (ABI 6).
#[repr(C)]
struct InitParams {
    abi_version: i32,
    talker_path: *const c_char,
    codec_path: *const c_char,
    use_fa: bool,
    clamp_fp16: bool,
    max_batch: i32,
    codec_chunk_sec: f32,
    device: *const c_char,
}

/// `struct qt_tts_params` of the pinned `qwen.h` (ABI 6).
#[repr(C)]
struct TtsParams {
    abi_version: i32,
    text: *const c_char,
    lang: *const c_char,
    instruct: *const c_char,
    speaker: *const c_char,
    ref_audio_24k: *const f32,
    ref_n_samples: i32,
    ref_text: *const c_char,
    seed: i64,
    max_new_tokens: i32,
    temperature: f32,
    top_k: i32,
    top_p: f32,
    repetition_penalty: f32,
    subtalker_temperature: f32,
    subtalker_top_k: i32,
    subtalker_top_p: f32,
    dump_dir: *const c_char,
    cancel: *const c_void,
    cancel_user_data: *mut c_void,
    on_chunk: *const c_void,
    on_chunk_user_data: *mut c_void,
    ref_spk_emb: *const f32,
    ref_spk_dim: i32,
    ref_codes: *const i32,
    ref_t: i32,
}

/// `struct qt_audio`.
#[repr(C)]
struct Audio {
    samples: *mut f32,
    n_samples: i32,
    sample_rate: i32,
    channels: i32,
}

type InitDefaults = unsafe extern "C" fn(*mut InitParams);
type Init = unsafe extern "C" fn(*const InitParams) -> *mut c_void;
type Free = unsafe extern "C" fn(*mut c_void);
type TtsDefaults = unsafe extern "C" fn(*mut TtsParams);
type Synthesize = unsafe extern "C" fn(*mut c_void, *const TtsParams, *mut Audio) -> i32;
type AudioFree = unsafe extern "C" fn(*mut Audio);
type LastError = unsafe extern "C" fn() -> *const c_char;
type LogCallback = unsafe extern "C" fn(i32, *const c_char, *mut c_void);
type LogSet = unsafe extern "C" fn(Option<LogCallback>, *mut c_void);

/// How the engine uses the device; chosen by the parent (see `Profile`).
pub struct Options<'a> {
    /// GGML device name (`CPU`) or `None` for the best available device.
    pub device: Option<&'a str>,
    /// Fused flash attention; off for GPUs where it faults (GTX 1060, GL-018).
    pub use_fa: bool,
    /// Guard FP16 accumulation on GPUs without native FP16 support.
    pub clamp_fp16: bool,
    /// Buffered codec decode chunk, seconds: smaller needs less video memory.
    pub codec_chunk_sec: f32,
}

/// Why one synthesis failed.
pub enum Failure {
    /// Out of device memory or an empty decode: retry with a safer profile.
    Device,
    /// Anything else.
    Other,
}

pub struct Engine {
    context: *mut c_void,
    free: Free,
    tts_defaults: TtsDefaults,
    synthesize: Synthesize,
    audio_free: AudioFree,
    last_error: LastError,
    _library: Library,
}

unsafe extern "C" fn quiet(_: i32, _: *const c_char, _: *mut c_void) {}

impl Engine {
    /// Load the verified runtime from `root/runtime` and both GGUF models.
    pub fn open(root: &Path, options: &Options<'_>) -> Result<Self, String> {
        super::verify(root)?;
        let dll = root.join(super::catalog::RUNTIME_DIR).join("qwen.dll");
        // SAFETY: verify() checked every pinned DLL and model hash. Dependencies
        // resolve only beside the verified DLL or in System32; ggml loads its
        // backend DLLs from the worker's current directory, the same verified
        // runtime folder.
        #[cfg(windows)]
        let library: Library = unsafe {
            libloading::os::windows::Library::load_with_flags(&dll, 0x100 | 0x800)
                .map_err(|e| e.to_string())?
                .into()
        };
        // SAFETY: same verified file; this path is not shipped outside Windows.
        #[cfg(not(windows))]
        let library = unsafe { Library::new(&dll).map_err(|e| e.to_string())? };
        // SAFETY: the signatures and struct layouts match the pinned qwen.h
        // (ABI 6); the defaults functions fill abi_version; the CStrings outlive
        // qt_init, which copies what it keeps.
        unsafe {
            let init_defaults = *library
                .get::<InitDefaults>(b"qt_init_default_params\0")
                .map_err(|e| e.to_string())?;
            let init = *library
                .get::<Init>(b"qt_init\0")
                .map_err(|e| e.to_string())?;
            let free = *library
                .get::<Free>(b"qt_free\0")
                .map_err(|e| e.to_string())?;
            let tts_defaults = *library
                .get::<TtsDefaults>(b"qt_tts_default_params\0")
                .map_err(|e| e.to_string())?;
            let synthesize = *library
                .get::<Synthesize>(b"qt_synthesize\0")
                .map_err(|e| e.to_string())?;
            let audio_free = *library
                .get::<AudioFree>(b"qt_audio_free\0")
                .map_err(|e| e.to_string())?;
            let last_error = *library
                .get::<LastError>(b"qt_last_error\0")
                .map_err(|e| e.to_string())?;
            let log_set = *library
                .get::<LogSet>(b"qt_log_set\0")
                .map_err(|e| e.to_string())?;
            // The library's own log never reaches Glagol's logs (it is not needed
            // and must not carry anything about the text).
            log_set(Some(quiet), std::ptr::null_mut());
            let path = |file: &str| {
                CString::new(root.join(file).to_str().ok_or("Invalid model path")?)
                    .map_err(|e| e.to_string())
            };
            let talker = path(super::catalog::TALKER.file)?;
            let codec = path(super::catalog::CODEC.file)?;
            let device = options
                .device
                .map(CString::new)
                .transpose()
                .map_err(|e| e.to_string())?;
            let mut params = std::mem::MaybeUninit::<InitParams>::zeroed();
            init_defaults(params.as_mut_ptr());
            let mut params = params.assume_init();
            params.talker_path = talker.as_ptr();
            params.codec_path = codec.as_ptr();
            params.use_fa = options.use_fa;
            params.clamp_fp16 = options.clamp_fp16;
            params.codec_chunk_sec = options.codec_chunk_sec;
            params.max_batch = 1;
            params.device = device.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
            let context = init(&params);
            if context.is_null() {
                return Err("init".into());
            }
            Ok(Self {
                context,
                free,
                tts_defaults,
                synthesize,
                audio_free,
                last_error,
                _library: library,
            })
        }
    }

    /// Synthesize one chunk with a named speaker and an optional style
    /// instruction; mono f32 PCM at 24 kHz.
    pub fn synthesize(
        &mut self,
        text: &str,
        language: &str,
        speaker: &str,
        style: Option<&str>,
    ) -> Result<Vec<f32>, Failure> {
        let text = CString::new(text).map_err(|_| Failure::Other)?;
        let language = CString::new(language).map_err(|_| Failure::Other)?;
        let speaker = CString::new(speaker).map_err(|_| Failure::Other)?;
        let style = style
            .map(CString::new)
            .transpose()
            .map_err(|_| Failure::Other)?;
        // SAFETY: the context is live until Drop; params come from the library's
        // defaults; the CStrings outlive the blocking call; the audio buffer is
        // owned by the library and released with qt_audio_free after copying.
        unsafe {
            let mut params = std::mem::MaybeUninit::<TtsParams>::zeroed();
            (self.tts_defaults)(params.as_mut_ptr());
            let mut params = params.assume_init();
            params.text = text.as_ptr();
            params.lang = language.as_ptr();
            params.speaker = speaker.as_ptr();
            // Optional style instruction (CustomVoice), e.g. a calm, even tone.
            params.instruct = style.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
            // A fixed seed keeps a re-synthesized document identical.
            params.seed = 42;
            let mut audio = Audio {
                samples: std::ptr::null_mut(),
                n_samples: 0,
                sample_rate: 0,
                channels: 0,
            };
            let status = (self.synthesize)(self.context, &params, &mut audio);
            if status != 0 {
                (self.audio_free)(&mut audio);
                let message = CStr::from_ptr((self.last_error)())
                    .to_string_lossy()
                    .to_lowercase();
                // -4 is QT_STATUS_OOM; an empty decode is a device allocation
                // failure in practice (2 GB cards, see the tts-qwen log).
                return Err(
                    if status == -4
                        || message.contains("no audio")
                        || message.contains("alloc")
                        || message.contains("memory")
                    {
                        Failure::Device
                    } else {
                        Failure::Other
                    },
                );
            }
            let samples = if audio.samples.is_null()
                || audio.n_samples <= 0
                || audio.sample_rate != 24_000
                || audio.channels != 1
            {
                Vec::new()
            } else {
                std::slice::from_raw_parts(audio.samples, audio.n_samples as usize).to_vec()
            };
            (self.audio_free)(&mut audio);
            Ok(samples)
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the context came from qt_init and is freed exactly once.
        unsafe { (self.free)(self.context) }
    }
}
