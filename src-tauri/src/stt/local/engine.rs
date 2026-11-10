//! Minimal bindings to the pinned transcribe.cpp 0.2.3 C ABI.
//! Header: https://github.com/handy-computer/transcribe.cpp/blob/v0.2.3/include/transcribe.h
//! Only one session computes at a time, behind LocalModels' mutex.
use super::catalog::RUNTIME_VERSION;
use libloading::Library;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    sync::{Arc, OnceLock},
};

#[repr(C)]
struct LoadParams {
    struct_size: u64,
    backend: i32,
    device: *mut c_void,
}
type Open =
    unsafe extern "C" fn(*const c_char, *const LoadParams, *const c_void, *mut *mut c_void) -> i32;
type Run = unsafe extern "C" fn(*mut c_void, *const f32, i32, *const c_void) -> i32;
type Text = unsafe extern "C" fn(*const c_void) -> *const c_char;
type Close = unsafe extern "C" fn(*mut c_void);
type GetTimings = unsafe extern "C" fn(*const c_void, *mut Timings) -> i32;
type TimingsInit = unsafe extern "C" fn(*mut Timings);

/// `struct transcribe_session_params` from the pinned 0.2.3 header.
#[repr(C)]
struct SessionParams {
    struct_size: u64,
    n_threads: i32,
    kv_type: i32,
    n_ctx: i32,
}

/// `TRANSCRIBE_BACKEND_CPU` / `TRANSCRIBE_BACKEND_VULKAN` from the 0.2.3 header.
const BACKEND_CPU: i32 = 1;
const BACKEND_VULKAN: i32 = 3;

/// `struct transcribe_timings` from the pinned 0.2.3 header.
#[repr(C)]
#[derive(Default)]
struct Timings {
    struct_size: u64,
    load_ms: f32,
    mel_ms: f32,
    encode_ms: f32,
    decode_ms: f32,
}

/// `struct transcribe_device_info` from the pinned 0.2.3 header.
#[repr(C)]
struct DeviceInfo {
    struct_size: u64,
    name: *const c_char,
    description: *const c_char,
    kind: *const c_char,
    device_id: *const c_char,
    memory_total: u64,
    memory_free: u64,
    device_type: i32,
}

/// Per-recording engine time, summed over segments (milliseconds).
#[derive(Debug, Default, Clone, Copy)]
pub struct RunStats {
    pub segments: usize,
    pub mel_ms: f32,
    pub encode_ms: f32,
    pub decode_ms: f32,
}

struct Native {
    _library: Library,
    open: Open,
    run: Run,
    text: Text,
    close: Close,
    timings: Option<(TimingsInit, GetTimings)>,
}
static NATIVE: OnceLock<Result<Arc<Native>, String>> = OnceLock::new();

/// Name of the first discrete Vulkan GPU the runtime registered, if any.
static DISCRETE_GPU: OnceLock<String> = OnceLock::new();

/// The discrete graphics card usable through Vulkan, known after the runtime
/// has been loaded once in this process (startup probe or first model load).
pub fn discrete_gpu() -> Option<String> {
    DISCRETE_GPU.get().cloned()
}

/// Whether the runtime was loaded and its devices enumerated in this process.
pub fn runtime_loaded() -> bool {
    NATIVE.get().is_some_and(|native| native.is_ok())
}

/// Load the verified runtime and enumerate compute devices without opening a
/// model. The caller must have verified the runtime files first.
pub fn probe(dir: &Path) -> Result<(), String> {
    Native::load(dir).map(|_| ())
}

/// Copy a runtime-owned C string for diagnostics; NULL becomes empty.
///
/// # Safety
/// `ptr` is NULL or a NUL-terminated string that stays valid during the call.
unsafe fn c_text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .chars()
            .take(160)
            .collect()
    }
}

/// Describe registered compute devices and the ggml CPU variant's features.
/// Hardware names and capability flags only; never user content.
fn log_backends(library: &Library, dir: &Path) {
    let mut devices = Vec::new();
    // SAFETY: symbols and struct layouts match the pinned 0.2.3 header. Device
    // handles and their strings are runtime-owned for the process lifetime;
    // each string is copied immediately.
    unsafe {
        let count: Result<libloading::Symbol<unsafe extern "C" fn() -> i32>, _> =
            library.get(b"transcribe_device_count\0");
        let get: Result<libloading::Symbol<unsafe extern "C" fn(i32) -> *mut c_void>, _> =
            library.get(b"transcribe_device_get\0");
        let init: Result<libloading::Symbol<unsafe extern "C" fn(*mut DeviceInfo)>, _> =
            library.get(b"transcribe_device_info_init\0");
        let info: Result<
            libloading::Symbol<unsafe extern "C" fn(*mut c_void, *mut DeviceInfo) -> i32>,
            _,
        > = library.get(b"transcribe_device_get_info\0");
        if let (Ok(count), Ok(get), Ok(init), Ok(info)) = (count, get, init, info) {
            for index in 0..count().clamp(0, 16) {
                let device = get(index);
                if device.is_null() {
                    continue;
                }
                let mut out = DeviceInfo {
                    struct_size: 0,
                    name: std::ptr::null(),
                    description: std::ptr::null(),
                    kind: std::ptr::null(),
                    device_id: std::ptr::null(),
                    memory_total: 0,
                    memory_free: 0,
                    device_type: -1,
                };
                init(&mut out);
                if info(device, &mut out) == 0 {
                    // TRANSCRIBE_DEVICE_TYPE_GPU (1) is a discrete card.
                    if out.device_type == 1 && c_text(out.kind) == "vulkan" {
                        let _ = DISCRETE_GPU.set(c_text(out.description).trim().to_string());
                    }
                    devices.push(format!(
                        "{}:{}:{} ({}, {} MB)",
                        index,
                        c_text(out.kind),
                        c_text(out.name),
                        c_text(out.description),
                        out.memory_total / 1_000_000
                    ));
                }
            }
        }
    }
    tracing::info!(
        devices = %devices.join("; "),
        cpu_variant_features = %ggml_cpu_features(dir),
        "local STT compute devices"
    );
}

/// Features compiled into the ggml CPU variant that the runtime selected
/// (e.g. `AVX2=1,FMA=1`). Empty when the registry cannot be queried.
fn ggml_cpu_features(dir: &Path) -> String {
    #[repr(C)]
    struct Feature {
        name: *const c_char,
        value: *const c_char,
    }
    type Features = unsafe extern "C" fn(*mut c_void) -> *const Feature;
    #[cfg(windows)]
    let file = dir.join("ggml.dll");
    #[cfg(not(windows))]
    let file = dir.join("libggml.so");
    // SAFETY: ggml.dll is part of the same hash-verified runtime and is already
    // loaded as a dependency of transcribe.dll, so this only adds a reference
    // to the existing module (same search flags as the main load). Symbols
    // match ggml's public backend registry API; the feature array is owned by
    // the backend and terminated by a NULL name. The handle is kept for the
    // process lifetime like the main library.
    unsafe {
        #[cfg(windows)]
        let library: Result<Library, _> =
            libloading::os::windows::Library::load_with_flags(file, 0x100 | 0x1000).map(Into::into);
        #[cfg(not(windows))]
        let library = Library::new(file);
        let Ok(library) = library else {
            return String::new();
        };
        let library: &'static Library = Box::leak(Box::new(library));
        let (Ok(count), Ok(get), Ok(name), Ok(proc)) = (
            library.get::<unsafe extern "C" fn() -> usize>(b"ggml_backend_reg_count\0"),
            library.get::<unsafe extern "C" fn(usize) -> *mut c_void>(b"ggml_backend_reg_get\0"),
            library.get::<unsafe extern "C" fn(*mut c_void) -> *const c_char>(
                b"ggml_backend_reg_name\0",
            ),
            library.get::<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>(
                b"ggml_backend_reg_get_proc_address\0",
            ),
        ) else {
            return String::new();
        };
        for index in 0..count().min(16) {
            let reg = get(index);
            if reg.is_null() || c_text(name(reg)) != "CPU" {
                continue;
            }
            let address = proc(reg, c"ggml_backend_get_features".as_ptr());
            if address.is_null() {
                return String::new();
            }
            let features: Features = std::mem::transmute::<*mut c_void, Features>(address);
            let mut list = Vec::new();
            let mut item = features(reg);
            while !item.is_null() && !(*item).name.is_null() && list.len() < 48 {
                let value = c_text((*item).value);
                if value != "0" {
                    list.push(format!("{}={}", c_text((*item).name), value));
                }
                item = item.add(1);
            }
            return list.join(",");
        }
        String::new()
    }
}

impl Native {
    fn load(dir: &Path) -> Result<Arc<Self>, String> {
        NATIVE
            .get_or_init(|| {
                let started = std::time::Instant::now();
                // SAFETY: dir contains the hash-verified pinned runtime. On Windows, resolve
                // dependencies only beside this DLL or in trusted system directories, never CWD.
                let library: Library = unsafe {
                    #[cfg(windows)]
                    {
                        libloading::os::windows::Library::load_with_flags(
                            dir.join("transcribe.dll"),
                            0x100 | 0x1000,
                        )
                        .map(Into::into)
                    }
                    #[cfg(not(windows))]
                    {
                        Library::new(dir.join("libtranscribe.so"))
                    }
                }
                .map_err(|e| format!("Не удалось загрузить движок диктовки: {e}"))?;
                // SAFETY: symbols and layouts match the pinned 0.2.3 header. Library is
                // retained for process lifetime because ggml keeps global backend registrations.
                unsafe {
                    let version: libloading::Symbol<unsafe extern "C" fn() -> *const c_char> =
                        library
                            .get(b"transcribe_version\0")
                            .map_err(|e| e.to_string())?;
                    if CStr::from_ptr(version()).to_bytes() != RUNTIME_VERSION.as_bytes() {
                        return Err("Несовместимая версия движка диктовки.".into());
                    }
                    type LogSet = unsafe extern "C" fn(
                        Option<unsafe extern "C" fn(i32, *const c_char, *mut c_void)>,
                        *mut c_void,
                    );
                    let log: libloading::Symbol<LogSet> = library
                        .get(b"transcribe_log_set\0")
                        .map_err(|e| e.to_string())?;
                    // NULL explicitly silences both transcribe and ggml; never log user speech.
                    log(None, std::ptr::null_mut());
                    let init: libloading::Symbol<unsafe extern "C" fn() -> i32> = library
                        .get(b"transcribe_init_backends_default\0")
                        .map_err(|e| e.to_string())?;
                    if init() != 0 {
                        return Err("Не удалось запустить CPU-движок диктовки.".into());
                    }
                    tracing::info!(
                        init_ms = started.elapsed().as_millis() as u64,
                        "local STT runtime initialized"
                    );
                    log_backends(&library, dir);
                    let timings = match (
                        library.get::<TimingsInit>(b"transcribe_timings_init\0"),
                        library.get::<GetTimings>(b"transcribe_get_timings\0"),
                    ) {
                        (Ok(init), Ok(get)) => Some((*init, *get)),
                        _ => None,
                    };
                    Ok(Arc::new(Self {
                        timings,
                        open: *library
                            .get(b"transcribe_open\0")
                            .map_err(|e| e.to_string())?,
                        run: *library
                            .get(b"transcribe_run\0")
                            .map_err(|e| e.to_string())?,
                        text: *library
                            .get(b"transcribe_full_text\0")
                            .map_err(|e| e.to_string())?,
                        close: *library
                            .get(b"transcribe_session_free\0")
                            .map_err(|e| e.to_string())?,
                        _library: library,
                    }))
                }
            })
            .clone()
    }
}

pub struct Engine {
    native: Arc<Native>,
    session: *mut c_void,
    pub model_id: String,
    /// The graphics card (Vulkan) was requested for this session.
    pub gpu_requested: bool,
    /// Requested CPU threads; 0 lets the library choose.
    pub threads: u32,
    /// Backend the library bound, e.g. `CPU` or `Vulkan0`.
    pub backend: String,
    /// Vulkan was requested but could not open, so the CPU is used.
    pub fell_back: bool,
}
// SAFETY: the C API permits moving a session across threads, but not concurrent
// access. Engine is only accessed through an exclusive Mutex guard. It is not Sync.
unsafe impl Send for Engine {}

impl Engine {
    /// Open `model_path` on the graphics card (Vulkan) when `gpu` is set, falling
    /// back to the CPU if that fails; `threads == 0` keeps the library default.
    pub fn open(
        runtime: &Path,
        model_path: &Path,
        model_id: &str,
        gpu: bool,
        threads: u32,
    ) -> Result<Self, String> {
        let native = Native::load(runtime)?;
        let path = CString::new(
            model_path
                .to_str()
                .ok_or("Путь модели не является UTF-8.")?,
        )
        .map_err(|e| e.to_string())?;
        let mut fell_back = false;
        let session = if gpu {
            let started = std::time::Instant::now();
            match Self::try_open(&native, &path, BACKEND_VULKAN, threads) {
                Ok(session) => session,
                Err(status) => {
                    tracing::warn!(
                        model = model_id,
                        status,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "local STT graphics card unavailable; using CPU"
                    );
                    fell_back = true;
                    Self::try_open(&native, &path, BACKEND_CPU, threads)
                        .map_err(Self::open_error)?
                }
            }
        } else {
            Self::try_open(&native, &path, BACKEND_CPU, threads).map_err(Self::open_error)?
        };
        // SAFETY: the session is valid; the model and backend string are
        // library-owned for the session lifetime and copied immediately.
        let backend = unsafe {
            let model: Result<
                libloading::Symbol<unsafe extern "C" fn(*const c_void) -> *const c_void>,
                _,
            > = native._library.get(b"transcribe_get_model\0");
            let name: Result<
                libloading::Symbol<unsafe extern "C" fn(*const c_void) -> *const c_char>,
                _,
            > = native._library.get(b"transcribe_model_backend\0");
            match (model, name) {
                (Ok(model), Ok(name)) => c_text(name(model(session))),
                _ => String::new(),
            }
        };
        tracing::info!(
            model = model_id,
            backend = %backend,
            gpu_requested = gpu,
            fell_back,
            threads = if threads == 0 { crate::diagnostics::stt_default_threads() as u32 } else { threads },
            "local STT session opened"
        );
        Ok(Self {
            native,
            session,
            model_id: model_id.into(),
            gpu_requested: gpu,
            threads,
            backend,
            fell_back,
        })
    }

    fn open_error(status: i32) -> String {
        format!("Не удалось открыть модель (код {status}). Попробуйте скачать её заново.")
    }

    /// One `transcribe_open` attempt; returns the library status on failure.
    fn try_open(
        native: &Native,
        path: &CString,
        backend: i32,
        threads: u32,
    ) -> Result<*mut c_void, i32> {
        let mut params = LoadParams {
            struct_size: 0,
            backend: 0,
            device: std::ptr::null_mut(),
        };
        let mut session_params = SessionParams {
            struct_size: 0,
            n_threads: 0,
            kv_type: 0,
            n_ctx: 0,
        };
        let mut session = std::ptr::null_mut();
        // SAFETY: both param structs have the exact 0.2.3 C layout and are
        // initialized through the API before the backend and thread count are
        // set; a NULL device selects the first device of the requested backend.
        // C strings and output storage live throughout open.
        let status = unsafe {
            let init: libloading::Symbol<unsafe extern "C" fn(*mut LoadParams)> = native
                ._library
                .get(b"transcribe_model_load_params_init\0")
                .map_err(|_| -1)?;
            init(&mut params);
            params.backend = backend;
            let session_init: libloading::Symbol<unsafe extern "C" fn(*mut SessionParams)> = native
                ._library
                .get(b"transcribe_session_params_init\0")
                .map_err(|_| -1)?;
            session_init(&mut session_params);
            session_params.n_threads = threads.min(64) as i32;
            (native.open)(
                path.as_ptr(),
                &params,
                (&session_params as *const SessionParams).cast(),
                &mut session,
            )
        };
        if status != 0 || session.is_null() {
            return Err(if status == 0 { -1 } else { status });
        }
        Ok(session)
    }

    /// Recognize 16 kHz mono PCM; also returns engine time per stage.
    pub fn transcribe(&mut self, pcm: &[f32]) -> Result<(String, RunStats), String> {
        let mut parts = Vec::new();
        let mut stats = RunStats::default();
        for range in segments(pcm) {
            let samples = &pcm[range];
            // SAFETY: exclusive session access; input is bounded <=24s of valid
            // 16kHz mono f32 PCM and lives until run returns. NULL run params are supported.
            let status = unsafe {
                (self.native.run)(
                    self.session,
                    samples.as_ptr(),
                    samples.len() as i32,
                    std::ptr::null(),
                )
            };
            if status != 0 {
                return Err(format!("Ошибка локального распознавания (код {status})."));
            }
            // SAFETY: text is session-owned and valid until next run; copy immediately.
            let text = unsafe {
                let ptr = (self.native.text)(self.session);
                if ptr.is_null() {
                    return Err("Движок не вернул текст.".into());
                }
                CStr::from_ptr(ptr).to_string_lossy().trim().to_owned()
            };
            stats.segments += 1;
            if let Some((init, get)) = self.native.timings {
                let mut timings = Timings::default();
                // SAFETY: exclusive session access; `timings` is initialized by
                // the library's own init, then filled within its struct_size.
                if unsafe {
                    init(&mut timings);
                    get(self.session, &mut timings)
                } == 0
                {
                    stats.mel_ms += timings.mel_ms;
                    stats.encode_ms += timings.encode_ms;
                    stats.decode_ms += timings.decode_ms;
                }
            }
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Ok((parts.join(" "), stats))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: owns exactly one session from transcribe_open; mutex ensures no
        // in-flight run can overlap teardown. Native library outlives this call.
        unsafe {
            (self.native.close)(self.session);
        }
    }
}

/// Cover all audio exactly once. For long recordings choose a quiet boundary
/// between 16 and 24 seconds; never split short utterances on thinking pauses.
pub fn segments(pcm: &[f32]) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    const MAX: usize = 24 * 16_000;
    while pcm.len() - start > MAX {
        let begin = start + 16 * 16_000;
        let end = start + MAX;
        let split = (begin..end)
            .step_by(1600)
            .min_by(|&a, &b| {
                let energy = |p: usize| pcm[p - 800..p + 800].iter().map(|v| v * v).sum::<f32>();
                energy(a).total_cmp(&energy(b))
            })
            .unwrap_or(end);
        ranges.push(start..split);
        start = split;
    }
    if start < pcm.len() {
        ranges.push(start..pcm.len());
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments_cover_long_audio_without_loss_or_overlap() {
        let mut pcm = vec![0.2; 65 * 16_000];
        pcm[20 * 16_000 - 800..20 * 16_000 + 800].fill(0.0);
        let ranges = segments(&pcm);
        assert_eq!(ranges[0].end, 20 * 16_000);
        assert_eq!(ranges.iter().map(|r| r.len()).sum::<usize>(), pcm.len());
        assert!(ranges.iter().all(|r| r.len() <= 24 * 16_000));
        assert!(ranges.windows(2).all(|w| w[0].end == w[1].start));
    }
    #[test]
    fn short_utterance_is_not_split_at_pauses() {
        assert_eq!(segments(&vec![0.0; 10 * 16_000]), vec![0..160_000]);
        assert!(segments(&[]).is_empty());
    }
}
