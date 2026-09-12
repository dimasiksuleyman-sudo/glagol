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

struct Native {
    _library: Library,
    open: Open,
    run: Run,
    text: Text,
    close: Close,
}
static NATIVE: OnceLock<Result<Arc<Native>, String>> = OnceLock::new();

impl Native {
    fn load(dir: &Path) -> Result<Arc<Self>, String> {
        NATIVE
            .get_or_init(|| {
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
                    Ok(Arc::new(Self {
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
}
// SAFETY: the C API permits moving a session across threads, but not concurrent
// access. Engine is only accessed through an exclusive Mutex guard. It is not Sync.
unsafe impl Send for Engine {}

impl Engine {
    pub fn open(runtime: &Path, model_path: &Path, model_id: &str) -> Result<Self, String> {
        let native = Native::load(runtime)?;
        let path = CString::new(
            model_path
                .to_str()
                .ok_or("Путь модели не является UTF-8.")?,
        )
        .map_err(|e| e.to_string())?;
        let mut params = LoadParams {
            struct_size: 0,
            backend: 0,
            device: std::ptr::null_mut(),
        };
        let mut session = std::ptr::null_mut();
        // SAFETY: params has the exact C layout; initialize through the API, then
        // request CPU explicitly. C strings and output storage live throughout open.
        let status = unsafe {
            let init: libloading::Symbol<unsafe extern "C" fn(*mut LoadParams)> = native
                ._library
                .get(b"transcribe_model_load_params_init\0")
                .map_err(|e| e.to_string())?;
            init(&mut params);
            params.backend = 1;
            (native.open)(path.as_ptr(), &params, std::ptr::null(), &mut session)
        };
        if status != 0 || session.is_null() {
            return Err(format!(
                "Не удалось открыть модель (код {status}). Попробуйте скачать её заново."
            ));
        }
        Ok(Self {
            native,
            session,
            model_id: model_id.into(),
        })
    }

    pub fn transcribe(&mut self, pcm: &[f32]) -> Result<String, String> {
        let mut parts = Vec::new();
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
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Ok(parts.join(" "))
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
