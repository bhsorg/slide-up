use crate::error::{AppError, Result};
use crate::mpv::ffi::{
    MpvFfi, MPV_EVENT_END_FILE, MPV_FORMAT_DOUBLE, MPV_FORMAT_INT64,
};
use std::ffi::{c_char, c_void, CString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Number of frames cached per video for seek-preview thumbnails.
pub const STORYBOARD_FRAMES: usize = 12;

/// Cached thumbnails and storyboard frames are generated at exactly the size
/// used by the seek-preview tooltip, so no resizing is needed at display time.
pub const THUMBNAIL_WIDTH: i32 = 160;
pub const THUMBNAIL_HEIGHT: i32 = 90;
/// JPEG keeps the encrypted `.hlv` cache small; PNG frames were ~20KB each.
const IMAGE_FORMAT: &str = "jpg";
const JPEG_QUALITY: &str = "78";

/// Called with the number of captured frames and the total frame count while
/// a storyboard is being generated. Must be thread-safe because probing runs
/// on a blocking thread.
pub type StoryboardProgress = dyn Fn(usize, usize) + Send + Sync;

#[derive(Debug, Default)]
pub struct VideoMetadata {
    pub duration_ms: i64,
    pub width: i64,
    pub height: i64,
    pub codec: Option<String>,
}

struct Probe {
    ffi: std::sync::Arc<MpvFfi>,
    handle: *mut c_void,
}

impl Probe {
    fn new() -> Result<Self> {
        let ffi = MpvFfi::load()?;
        let handle = unsafe { (ffi.create)() };
        if handle.is_null() {
            return Err(AppError::media("Could not inspect this video file."));
        }
        let code = unsafe { (ffi.initialize)(handle) };
        ffi.check(code)?;
        Ok(Self { ffi, handle })
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        let name = MpvFfi::cstring(name)?;
        let value = MpvFfi::cstring(value)?;
        let code = unsafe {
            (self.ffi.set_property_string)(self.handle, name.as_ptr(), value.as_ptr())
        };
        self.ffi.check(code)
    }

    fn get_i64(&self, name: &str) -> i64 {
        let name = match MpvFfi::cstring(name) {
            Ok(name) => name,
            Err(_) => return 0,
        };
        let mut value: i64 = 0;
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_INT64,
                &mut value as *mut i64 as *mut c_void,
            )
        };
        if code >= 0 { value } else { 0 }
    }

    fn get_f64(&self, name: &str) -> f64 {
        let name = match MpvFfi::cstring(name) {
            Ok(name) => name,
            Err(_) => return 0.0,
        };
        let mut value: f64 = 0.0;
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut value as *mut f64 as *mut c_void,
            )
        };
        if code >= 0 { value } else { 0.0 }
    }

    fn get_string(&self, name: &str) -> Option<String> {
        let name = MpvFfi::cstring(name).ok()?;
        let ptr = unsafe { (self.ffi.get_property_string)(self.handle, name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let value = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
        unsafe { (self.ffi.free)(ptr as *mut c_void) };
        Some(value)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        unsafe {
            (self.ffi.destroy)(self.handle);
        }
    }
}

/// Temporary staging area for freshly decoded storyboard frames. It lives in
/// the OS temp directory, never under the app data directory, because these
/// PNGs are plaintext until `persist_storyboard` encrypts them into `.hlv`.
pub fn storyboard_staging_dir(media_id: i64) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "slideup-storyboard-{}-{}",
        std::process::id(),
        media_id
    ))
}

/// Reads video information using libmpv. When `generate_storyboard` is true,
/// also captures the cached seek-preview frames. No video cover image is
/// generated or stored — the UI falls back to a placeholder instead.
pub fn probe(
    path: &Path,
    media_id: i64,
    generate_storyboard: bool,
    progress: Option<Arc<StoryboardProgress>>,
) -> Result<VideoMetadata> {
    let temp_dir = std::env::temp_dir().join(format!(
        "slideup-thumb-{}-{}",
        media_id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::create_dir_all(&temp_dir)?;

    // Frames are staged in the OS temp directory and encrypted into the
    // workspace cache by the caller once the real database id is known.
    let story_dir = storyboard_staging_dir(media_id);
    if generate_storyboard {
        let _ = std::fs::remove_dir_all(&story_dir);
        let _ = std::fs::create_dir_all(&story_dir);
    }

    let probe = Probe::new()?;
    probe.set("vo", "image")?;
    // Scale frames while probing so cached storyboard frames are 160x90.
    // The lavfi graph keeps the source aspect ratio and
    // pads with black bars; fall back to mpv's builtin scaler if unavailable.
    let graph = format!(
        "lavfi=[scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black]",
        THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT
    );
    if probe.set("vf", &graph).is_err() {
        let _ = probe.set(
            "vf",
            &format!("scale={}:{}", THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT),
        );
    }
    probe.set("vo-image-format", IMAGE_FORMAT)?;
    probe.set("vo-image-jpeg-quality", JPEG_QUALITY)?;
    probe.set("vo-image-outdir", &temp_dir.to_string_lossy())?;
    probe.set("frames", "1")?;
    probe.set("start", "5")?;
    probe.set("audio", "no")?;
    probe.set("sub", "no")?;
    probe.set("hwdec", "no")?;
    probe.set("terminal", "no")?;
    probe.set("msg-level", "all=error")?;
    probe.set("osd-level", "0")?;

    let path_string = path.to_string_lossy().into_owned();
    let load = CString::new("loadfile").unwrap();
    let file = CString::new(path_string.as_str())
        .map_err(|_| AppError::media("The media path contains an invalid character."))?;
    let replace = CString::new("replace").unwrap();
    let pointers: [*const c_char; 4] = [
        load.as_ptr(),
        file.as_ptr(),
        replace.as_ptr(),
        std::ptr::null(),
    ];
    let code = unsafe { (probe.ffi.command)(probe.handle, pointers.as_ptr()) };
    probe.ffi.check(code)?;

    // Capture metadata while the file is still loaded. After END_FILE mpv may
    // clear properties because `keep-open` is off, so reading them afterwards
    // can yield zeroes even though the probe succeeded.
    let started = Instant::now();
    let mut metadata = VideoMetadata::default();
    let mut ended = false;
    while started.elapsed().as_secs() < 20 {
        let event = unsafe { (probe.ffi.wait_event)(probe.handle, 0.05) };
        if !event.is_null() && unsafe { (*event).event_id } == MPV_EVENT_END_FILE {
            ended = true;
        }
        if metadata.duration_ms == 0 {
            let duration = (probe.get_f64("duration") * 1000.0).round() as i64;
            let width = probe.get_i64("video-params/w");
            if duration > 0 && width > 0 {
                metadata = VideoMetadata {
                    duration_ms: duration,
                    width,
                    height: probe.get_i64("video-params/h"),
                    codec: probe.get_string("video-codec"),
                };
            }
        }
        if ended && metadata.duration_ms > 0 {
            break;
        }
    }
    // Give the image VO a moment to finish writing the frame after EOF.
    if ended {
        std::thread::sleep(Duration::from_millis(120));
    }

    // The one decoded frame is only used as storyboard frame 0. It is never
    // copied into the `thumbnails` cache, which is reserved for audio artwork.
    if let Some(source) = find_image(&temp_dir) {
        if generate_storyboard {
            let _ = std::fs::copy(&source, story_dir.join("0.jpg"));
        }
        // Free the name so following captures can reuse the same image VO.
        let _ = std::fs::remove_file(&source);
    }

    if generate_storyboard && metadata.duration_ms > 0 {
        if let Some(report) = &progress {
            report(1, STORYBOARD_FRAMES);
        }
        capture_storyboard(
            &probe,
            path,
            metadata.duration_ms,
            &temp_dir,
            &story_dir,
            progress.as_deref(),
        );
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(metadata)
}

/// Captures the remaining seek-preview frames with the lightweight image VO.
/// This runs only during library import and never touches the playing instance.
fn capture_storyboard(
    probe: &Probe,
    path: &Path,
    duration_ms: i64,
    temp_dir: &Path,
    story_dir: &Path,
    progress: Option<&(dyn Fn(usize, usize) + Send + Sync)>,
) {
    let file = match CString::new(path.to_string_lossy().into_owned()) {
        Ok(file) => file,
        Err(_) => return,
    };
    let load = CString::new("loadfile").unwrap();
    let replace = CString::new("replace").unwrap();

    for index in 1..STORYBOARD_FRAMES {
        let seconds = duration_ms as f64 / 1000.0 * (index as f64 / STORYBOARD_FRAMES as f64);
        if probe.set("start", &format!("{seconds:.3}")).is_err() {
            break;
        }
        let pointers: [*const c_char; 4] = [
            load.as_ptr(),
            file.as_ptr(),
            replace.as_ptr(),
            std::ptr::null(),
        ];
        let code = unsafe { (probe.ffi.command)(probe.handle, pointers.as_ptr()) };
        if probe.ffi.check(code).is_err() {
            break;
        }

        let started = Instant::now();
        let mut ended = false;
        while started.elapsed().as_secs() < 6 {
            let event = unsafe { (probe.ffi.wait_event)(probe.handle, 0.05) };
            if !event.is_null() && unsafe { (*event).event_id } == MPV_EVENT_END_FILE {
                ended = true;
                break;
            }
        }
        if !ended {
            break;
        }
        // Give the image VO a moment to flush the PNG.
        std::thread::sleep(Duration::from_millis(100));

        let Some(source) = find_image(temp_dir) else {
            break;
        };
        let target = story_dir.join(format!("{index}.jpg"));
        if std::fs::rename(&source, &target).is_err() {
            break;
        }
        if let Some(report) = progress {
            report(index + 1, STORYBOARD_FRAMES);
        }
    }
}

/// Decode one small frame for a workspace-protected video cover. The seek
/// point is 10% of the duration (clamped to 1–30s) so short clips and long
/// videos do not produce black intro frames.
pub fn capture_cover(path: &Path, duration_ms: i64, temp_dir: &Path) -> Result<Vec<u8>> {
    let seconds = if duration_ms > 0 {
        ((duration_ms as f64 / 1000.0) * 0.1).clamp(1.0, 30.0)
    } else {
        5.0
    };

    let probe = Probe::new()?;
    probe.set("vo", "image")?;
    let graph = format!(
        "lavfi=[scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2:black]",
        THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT
    );
    if probe.set("vf", &graph).is_err() {
        let _ = probe.set("vf", &format!("scale={}:{}", THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT));
    }
    probe.set("vo-image-format", IMAGE_FORMAT)?;
    probe.set("vo-image-jpeg-quality", JPEG_QUALITY)?;
    probe.set("vo-image-outdir", &temp_dir.to_string_lossy())?;
    probe.set("frames", "1")?;
    probe.set("start", &format!("{seconds:.3}"))?;
    probe.set("audio", "no")?;
    probe.set("sub", "no")?;
    probe.set("hwdec", "no")?;
    probe.set("terminal", "no")?;
    probe.set("msg-level", "all=error")?;
    probe.set("osd-level", "0")?;

    let load = CString::new("loadfile").unwrap();
    let file = CString::new(path.to_string_lossy().into_owned())
        .map_err(|_| AppError::media("The media path contains an invalid character."))?;
    let replace = CString::new("replace").unwrap();
    let pointers: [*const c_char; 4] = [
        load.as_ptr(),
        file.as_ptr(),
        replace.as_ptr(),
        std::ptr::null(),
    ];
    let code = unsafe { (probe.ffi.command)(probe.handle, pointers.as_ptr()) };
    probe.ffi.check(code)?;

    let started = Instant::now();
    let mut ended = false;
    while started.elapsed().as_secs() < 20 {
        let event = unsafe { (probe.ffi.wait_event)(probe.handle, 0.05) };
        if !event.is_null() && unsafe { (*event).event_id } == MPV_EVENT_END_FILE {
            ended = true;
            break;
        }
    }
    if ended {
        std::thread::sleep(Duration::from_millis(120));
    }
    let source = find_image(temp_dir)
        .ok_or_else(|| AppError::media("Could not capture a frame from this video."))?;
    let bytes = std::fs::read(&source)?;
    let _ = std::fs::remove_file(&source);
    Ok(bytes)
}

fn find_image(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("png")
                        || ext.eq_ignore_ascii_case("jpg")
                        || ext.eq_ignore_ascii_case("jpeg")
                })
        })
}
