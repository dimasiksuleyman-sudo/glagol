//! The pinned ABI 30000 is accessed only inside a disposable worker process.
use libloading::Library;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
};

#[repr(C)]
struct OptionPair {
    name: *const c_char,
    value: *const c_char,
}
#[repr(C)]
struct Line {
    text: *const c_char,
    audio_data: *const f32,
    audio_data_count: usize,
    start_time: f32,
    duration: f32,
    id: u64,
    complete: i8,
    updated: i8,
    new: i8,
    text_changed: i8,
    speakers_changed: i8,
    speaker_spans: *const c_void,
    speaker_span_count: u64,
    latency_ms: u32,
    words: *const c_void,
    word_count: u64,
}
#[repr(C)]
struct Transcript {
    lines: *const Line,
    count: u64,
}
type Pair = unsafe extern "C" fn(i32, i32) -> i32;
type Free = unsafe extern "C" fn(i32);
type Create = unsafe extern "C" fn(i32, u32) -> i32;
type Add = unsafe extern "C" fn(i32, i32, *const f32, u64, i32, u32) -> i32;
type Decode = unsafe extern "C" fn(i32, i32, u32, *mut *const Transcript) -> i32;

pub struct Engine {
    handle: i32,
    stream: Option<i32>,
    create: Create,
    start: Pair,
    stop: Pair,
    free_stream: Pair,
    free: Free,
    add: Add,
    decode: Decode,
    _library: Library,
    _onnx: Library,
}
fn check(code: i32) -> Result<(), String> {
    if code < 0 {
        Err(format!("Moonshine engine error ({code})."))
    } else {
        Ok(())
    }
}
impl Engine {
    pub fn open(root: &Path) -> Result<Self, String> {
        super::verify(root)?;
        // Moonshine resolves ONNX dynamically. Preload the pinned absolute path
        // before Moonshine so a system-wide ORT cannot satisfy that lookup.
        // SAFETY: verify() checked every pinned DLL and model hash. Absolute
        // loading restricts dependencies to this directory and Windows System32.
        #[cfg(windows)]
        let onnx: Library = unsafe {
            libloading::os::windows::Library::load_with_flags(
                root.join("native/onnxruntime.dll"),
                0x100 | 0x800,
            )
            .map_err(|e| e.to_string())?
            .into()
        };
        // SAFETY: same pinned library lifetime; this path is not shipped on Windows.
        #[cfg(not(windows))]
        let onnx = unsafe {
            Library::new(root.join("native/onnxruntime.dll")).map_err(|e| e.to_string())?
        };
        // SAFETY: verified absolute DLL, held until every engine handle is freed.
        #[cfg(windows)]
        let library: Library = unsafe {
            // Dependencies can resolve only beside the verified DLL or in System32.
            libloading::os::windows::Library::load_with_flags(
                root.join("native/moonshine.dll"),
                0x100 | 0x800,
            )
            .map_err(|e| e.to_string())?
            .into()
        };
        // SAFETY: verify() precedes loading; Engine owns the library lifetime.
        #[cfg(not(windows))]
        let library =
            unsafe { Library::new(root.join("native/moonshine.dll")).map_err(|e| e.to_string())? };
        // SAFETY: signatures/layouts match the pinned ABI 30000 header. The ABI
        // version is checked before use; owned CStrings live throughout load().
        unsafe {
            let version = library
                .get::<unsafe extern "C" fn() -> i32>(b"moonshine_get_version\0")
                .map_err(|e| e.to_string())?;
            if version() != 30000 {
                return Err("Incompatible Moonshine runtime version.".into());
            }
            type Load =
                unsafe extern "C" fn(*const c_char, i32, *const OptionPair, u64, i32) -> i32;
            let load = *library
                .get::<Load>(b"moonshine_load_transcriber_from_files\0")
                .map_err(|e| e.to_string())?;
            // Resolve all symbols before allocating the model.
            let create = *library
                .get::<Create>(b"moonshine_create_stream\0")
                .map_err(|e| e.to_string())?;
            let start = *library
                .get::<Pair>(b"moonshine_start_stream\0")
                .map_err(|e| e.to_string())?;
            let stop = *library
                .get::<Pair>(b"moonshine_stop_stream\0")
                .map_err(|e| e.to_string())?;
            let free_stream = *library
                .get::<Pair>(b"moonshine_free_stream\0")
                .map_err(|e| e.to_string())?;
            let free = *library
                .get::<Free>(b"moonshine_free_transcriber\0")
                .map_err(|e| e.to_string())?;
            let add = *library
                .get::<Add>(b"moonshine_transcribe_add_audio_to_stream\0")
                .map_err(|e| e.to_string())?;
            let decode = *library
                .get::<Decode>(b"moonshine_transcribe_stream\0")
                .map_err(|e| e.to_string())?;
            let path = CString::new(root.join("stt").to_str().ok_or("Invalid model path")?)
                .map_err(|e| e.to_string())?;
            let options = [OptionPair {
                name: c"decode_incomplete_lines".as_ptr(),
                value: c"false".as_ptr(),
            }];
            let handle = load(
                path.as_ptr(),
                4,
                options.as_ptr(),
                options.len() as u64,
                30000,
            );
            check(handle)?;
            Ok(Self {
                handle,
                stream: None,
                create,
                start,
                stop,
                free_stream,
                free,
                add,
                decode,
                _library: library,
                _onnx: onnx,
            })
        }
    }
    pub fn begin(&mut self) -> Result<(), String> {
        self.reset();
        // SAFETY: self holds a live transcriber and its library.
        let stream = unsafe { (self.create)(self.handle, 0) };
        check(stream)?;
        self.stream = Some(stream);
        // SAFETY: create returned a valid owned stream, freed exactly once by reset.
        check(unsafe { (self.start)(self.handle, stream) })
    }
    pub fn audio(&mut self, samples: &[f32]) -> Result<(), String> {
        let stream = self.stream.ok_or("No active recognition stream")?;
        // SAFETY: samples is a live contiguous slice for the synchronous C call;
        // the active stream and transcriber remain owned by self.
        check(unsafe {
            (self.add)(
                self.handle,
                stream,
                samples.as_ptr(),
                samples.len() as u64,
                16000,
                0,
            )
        })?;
        // Decode while the key is held. Intermediate text never leaves the worker.
        let mut transcript = std::ptr::null();
        // SAFETY: valid handles and writable output pointer; no pointer retained here.
        check(unsafe { (self.decode)(self.handle, stream, 0, &mut transcript) })
    }
    pub fn finish(&mut self) -> Result<String, String> {
        let stream = self.stream.ok_or("No active recognition stream")?;
        // SAFETY: the active stream belongs to this transcriber.
        check(unsafe { (self.stop)(self.handle, stream) })?;
        let mut transcript = std::ptr::null();
        // SAFETY: valid stopped stream; decode returns a borrowed ABI transcript.
        check(unsafe { (self.decode)(self.handle, stream, 0, &mut transcript) })?;
        // SAFETY: the ABI guarantees line pointers/terminated strings until the
        // next engine call. Copy them before reset; reject null/excessive counts.
        let result = unsafe {
            if transcript.is_null() {
                return Err("Moonshine returned an invalid transcript.".into());
            }
            let transcript = &*transcript;
            if transcript.count > 4096 || (transcript.count != 0 && transcript.lines.is_null()) {
                return Err("Moonshine transcript exceeds the limit.".into());
            }
            let mut text = String::new();
            for index in 0..transcript.count as usize {
                let line = &*transcript.lines.add(index);
                if line.text.is_null() {
                    return Err("Moonshine returned an invalid line.".into());
                }
                let value = CStr::from_ptr(line.text)
                    .to_str()
                    .map_err(|_| "Invalid UTF-8 transcript")?
                    .trim();
                if !text.is_empty() && !value.is_empty() {
                    text.push(' ');
                }
                text.push_str(value);
                if text.len() > 60_000 {
                    return Err("Moonshine transcript exceeds the limit.".into());
                }
            }
            text
        };
        self.reset();
        Ok(result)
    }
    pub fn reset(&mut self) {
        if let Some(stream) = self.stream.take() {
            // SAFETY: take() transfers the sole owned stream; library is still live.
            unsafe {
                (self.free_stream)(self.handle, stream);
            }
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.reset();
        // SAFETY: sole owned handle, freed before the library fields are dropped.
        unsafe { (self.free)(self.handle) };
    }
}
