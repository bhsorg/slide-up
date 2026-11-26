//! Minimal, focused FFI bindings for the libmpv C API.
//!
//! The DLL is loaded at runtime with `libloading`, so the application builds
//! without libmpv present and shows a friendly error if the DLL is missing.

use crate::error::{AppError, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::PathBuf;
use std::sync::Arc;

pub const MPV_FORMAT_STRING: c_int = 1;
pub const MPV_FORMAT_FLAG: c_int = 3;
pub const MPV_FORMAT_INT64: c_int = 4;
pub const MPV_FORMAT_DOUBLE: c_int = 5;
pub const MPV_FORMAT_NODE: c_int = 6;

pub const MPV_EVENT_NONE: c_int = 0;
pub const MPV_EVENT_SHUTDOWN: c_int = 1;
pub const MPV_EVENT_FILE_LOADED: c_int = 8;
pub const MPV_EVENT_END_FILE: c_int = 7;
pub const MPV_EVENT_TRACKS_CHANGED: c_int = 9;
pub const MPV_EVENT_PAUSE: c_int = 12;
pub const MPV_EVENT_UNPAUSE: c_int = 13;
pub const MPV_EVENT_PROPERTY_CHANGE: c_int = 22;
pub const MPV_EVENT_PLAYBACK_RESTART: c_int = 21;

pub const END_REASON_EOF: c_int = 0;
pub const END_REASON_STOP: c_int = 2;
pub const END_REASON_QUIT: c_int = 3;
pub const END_REASON_ERROR: c_int = 4;

#[repr(C)]
#[derive(Copy, Clone)]
pub union MpvNodeValue {
    pub string: *mut c_char,
    pub flag: c_int,
    pub int64: i64,
    pub double_: f64,
    pub list: *mut MpvNodeList,
    pub byte_array: *mut MpvByteArray,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvNode {
    pub value: MpvNodeValue,
    pub format: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvNodeList {
    pub num: c_int,
    pub values: *mut MpvNode,
    pub keys: *mut *mut c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvByteArray {
    pub data: *mut c_void,
    pub size: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvEventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MpvEventEndFile {
    pub reason: c_int,
    pub error: c_int,
}

type MpvCreate = unsafe extern "C" fn() -> *mut c_void;
type MpvInitialize = unsafe extern "C" fn(*mut c_void) -> c_int;
type MpvDestroy = unsafe extern "C" fn(*mut c_void);
type MpvSetPropertyString = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type MpvSetProperty =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type MpvGetPropertyString = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;
type MpvGetProperty =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type MpvFree = unsafe extern "C" fn(*mut c_void);
type MpvFreeNodeContents = unsafe extern "C" fn(*mut MpvNode);
type MpvCommand = unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int;
type MpvWaitEvent = unsafe extern "C" fn(*mut c_void, f64) -> *mut MpvEvent;
type MpvObserveProperty = unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int;
type MpvEventName = unsafe extern "C" fn(c_int) -> *const c_char;
type MpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type MpvSetWakeupCallback =
    unsafe extern "C" fn(*mut c_void, unsafe extern "C" fn(*mut c_void), *mut c_void);
type MpvGetTimeUs = unsafe extern "C" fn(*mut c_void) -> u64;

pub struct MpvFfi {
    _library: Library,
    pub create: MpvCreate,
    pub initialize: MpvInitialize,
    pub destroy: MpvDestroy,
    pub set_property_string: MpvSetPropertyString,
    pub set_property: MpvSetProperty,
    pub get_property_string: MpvGetPropertyString,
    pub get_property: MpvGetProperty,
    pub free: MpvFree,
    pub free_node_contents: MpvFreeNodeContents,
    pub command: MpvCommand,
    pub wait_event: MpvWaitEvent,
    pub observe_property: MpvObserveProperty,
    pub event_name: MpvEventName,
    pub error_string: MpvErrorString,
    pub set_wakeup_callback: MpvSetWakeupCallback,
    pub get_time_us: MpvGetTimeUs,
}

// libloading::Library is safe to share between threads on all supported targets.
unsafe impl Send for MpvFfi {}
unsafe impl Sync for MpvFfi {}

impl MpvFfi {
    pub fn load() -> Result<Arc<Self>> {
        let path = locate_library().ok_or_else(|| {
            AppError::playback(
                "The media engine (libmpv-2.dll) was not found. Reinstall the application to restore it.",
            )
        })?;

        // Windows does not search a DLL's own directory for its dependencies.
        // Preload the Vulkan runtime shipped beside libmpv so LoadLibraryExW can
        // resolve it by module name regardless of the current directory or PATH.
        #[cfg(windows)]
        if let Some(parent) = path.parent() {
            let dependency = parent.join("vulkan-1.dll");
            if dependency.is_file() {
                if let Ok(library) = unsafe { Library::new(&dependency) } {
                    std::mem::forget(library);
                }
            }
        }

        unsafe {
            let library = Library::new(&path).map_err(|err| {
                let os = std::io::Error::last_os_error();
                AppError::playback(format!(
                    "Could not load the media engine at {}: {err} (OS error {})",
                    path.display(),
                    os.raw_os_error().unwrap_or(-1)
                ))
            })?;

            macro_rules! sym {
                ($name:literal, $ty:ty) => {
                    load_symbol::<$ty>(&library, $name)?
                };
            }

            let create = sym!("mpv_create", MpvCreate);
            let initialize = sym!("mpv_initialize", MpvInitialize);
            let destroy = sym!("mpv_destroy", MpvDestroy);
            let set_property_string = sym!("mpv_set_property_string", MpvSetPropertyString);
            let set_property = sym!("mpv_set_property", MpvSetProperty);
            let get_property_string = sym!("mpv_get_property_string", MpvGetPropertyString);
            let get_property = sym!("mpv_get_property", MpvGetProperty);
            let free = sym!("mpv_free", MpvFree);
            let free_node_contents = sym!("mpv_free_node_contents", MpvFreeNodeContents);
            let command = sym!("mpv_command", MpvCommand);
            let wait_event = sym!("mpv_wait_event", MpvWaitEvent);
            let observe_property = sym!("mpv_observe_property", MpvObserveProperty);
            let event_name = sym!("mpv_event_name", MpvEventName);
            let error_string = sym!("mpv_error_string", MpvErrorString);
            let set_wakeup_callback = sym!("mpv_set_wakeup_callback", MpvSetWakeupCallback);
            let get_time_us = sym!("mpv_get_time_us", MpvGetTimeUs);

            let ffi = Self {
                _library: library,
                create,
                initialize,
                destroy,
                set_property_string,
                set_property,
                get_property_string,
                get_property,
                free,
                free_node_contents,
                command,
                wait_event,
                observe_property,
                event_name,
                error_string,
                set_wakeup_callback,
                get_time_us,
            };
            Ok(Arc::new(ffi))
        }
    }

    pub fn error_message(&self, code: c_int) -> String {
        unsafe {
            let ptr = (self.error_string)(code);
            if ptr.is_null() {
                format!("libmpv error {code}")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    pub fn event_label(&self, id: c_int) -> String {
        unsafe {
            let ptr = (self.event_name)(id);
            if ptr.is_null() {
                format!("event {id}")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    pub fn check(&self, code: c_int) -> Result<()> {
        if code >= 0 {
            Ok(())
        } else {
            Err(AppError::playback(self.error_message(code)))
        }
    }

    pub fn cstring(value: &str) -> Result<CString> {
        CString::new(value)
            .map_err(|_| AppError::playback("A media path contains an invalid null character."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;
    use std::time::Duration;

    /// Smokes the bundled DLL: creates a libmpv instance and verifies that a
    /// real WAV file actually advances the playback clock.
    #[test]
    fn bundled_libmpv_decodes_audio() {
        let ffi = MpvFfi::load().expect("resources/libmpv-2.dll must be present for tests");
        unsafe {
            let handle = (ffi.create)();
            assert!(!handle.is_null(), "mpv_create failed");

            let set = |name: &str, value: &str| {
                let name = CString::new(name).unwrap();
                let value = CString::new(value).unwrap();
                ffi.check((ffi.set_property_string)(handle, name.as_ptr(), value.as_ptr())).unwrap();
            };
            set("ao", "null");
            // Optional manual diagnostics: set SLIDEUP_TEST_VO / SLIDEUP_TEST_VIDEO
            // to exercise a real video output and decode path.
            let vo = std::env::var("SLIDEUP_TEST_VO").unwrap_or_else(|_| "null".to_string());
            set("vo", &vo);
            set("idle", "yes");
            set("terminal", "no");
            set("msg-level", "all=error");
            ffi.check((ffi.initialize)(handle)).unwrap();

            let wav = write_test_wav();
            let video = std::env::var("SLIDEUP_TEST_VIDEO").ok();
            let media_path = video.as_deref().unwrap_or_else(|| wav.to_str().unwrap());
            let path = std::path::Path::new(media_path).to_string_lossy();
            let command = CString::new("loadfile").unwrap();
            let file = CString::new(path.as_ref()).unwrap();
            let mode = CString::new("replace").unwrap();
            let args = [command.as_ptr(), file.as_ptr(), mode.as_ptr(), ptr::null()];
            ffi.check((ffi.command)(handle, args.as_ptr())).unwrap();

            // Poll: DLL load time and decoder startup vary between runs.
            let name = CString::new("time-pos").unwrap();
            let mut position: f64 = -1.0;
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(100));
                let mut current: f64 = 0.0;
                let code = (ffi.get_property)(
                    handle,
                    name.as_ptr(),
                    MPV_FORMAT_DOUBLE,
                    &mut current as *mut f64 as *mut c_void,
                );
                if code >= 0 {
                    position = current;
                    if position > 0.05 {
                        break;
                    }
                }
            }
            assert!(position > 0.0, "mpv did not advance playback: {position}s");

            (ffi.destroy)(handle);
            let _ = std::fs::remove_file(wav);
        }
    }

    /// Manual diagnostic: exercises the exact metadata-probe path used by the
    /// library scanner. Run with:
    /// `SLIDEUP_TEST_VIDEO=<file> cargo test --lib -- --ignored --nocapture`
    #[test]
    #[ignore = "requires a real video file via SLIDEUP_TEST_VIDEO"]
    fn bundled_libmpv_probes_video_metadata() {
        let video = std::env::var("SLIDEUP_TEST_VIDEO")
            .expect("set SLIDEUP_TEST_VIDEO to a video file");
        let result = crate::metadata::video::probe(
            std::path::Path::new(&video),
            1,
            false,
            None,
        )
        .expect("video probe failed");
        eprintln!(
            "probe: duration={}ms {}x{} codec={:?}",
            result.duration_ms, result.width, result.height, result.codec
        );
        assert!(result.duration_ms > 0, "no duration reported");
    }

    fn write_test_wav() -> PathBuf {
        let sample_rate = 8000u32;
        let samples = sample_rate * 10; // ten seconds; survives slow decoder startup
        let data_len = samples * 2;
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..samples {
            let value = ((i as f64 / sample_rate as f64 * 440.0 * std::f64::consts::TAU).sin() * 12000.0) as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let path = std::env::temp_dir().join("slideup-mpv-smoke.wav");
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

fn load_symbol<T: Copy>(library: &Library, name: &str) -> Result<T> {
    unsafe {
        let symbol: Symbol<T> = library.get(name.as_bytes()).map_err(|err| {
            AppError::playback(format!(
                "The media engine is missing a required function ({name}): {err}"
            ))
        })?;
        Ok(*symbol)
    }
}

fn locate_library() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SLIDEUP_LIBMPV") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let candidates = [
        exe_dir.join("libmpv-2.dll"),
        exe_dir.join("resources").join("libmpv-2.dll"),
        // During `tauri dev` the crate directory is the resource root.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("libmpv-2.dll"),
        exe_dir.join("libmpv.dll"),
        PathBuf::from("libmpv-2.dll"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}
