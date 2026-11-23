use super::ffi::{
    MpvEventEndFile, MpvEventProperty, MpvFfi, MpvNode, END_REASON_ERROR,
    END_REASON_EOF, MPV_EVENT_END_FILE, MPV_EVENT_FILE_LOADED, MPV_EVENT_NONE,
    MPV_EVENT_PAUSE, MPV_EVENT_PROPERTY_CHANGE, MPV_EVENT_SHUTDOWN, MPV_EVENT_TRACKS_CHANGED,
    MPV_EVENT_UNPAUSE, MPV_FORMAT_DOUBLE, MPV_FORMAT_FLAG, MPV_FORMAT_INT64, MPV_FORMAT_NODE,
    MPV_FORMAT_STRING,
};
use crate::error::{AppError, Result};
use crate::events;
use crate::models::{
    AppSettings, AudioDevice, MediaItem, MediaTrack, PlayMode, PlayerSnapshot, PlayerStatus,
    TrackInfo,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const MPV_FORMAT_NODE_MAP: c_int = 8;
const MPV_FORMAT_NODE_ARRAY: c_int = 7;

#[cfg(windows)]
type Surface = super::native::VideoWindow;

#[cfg(not(windows))]
struct Surface;
#[cfg(not(windows))]
impl Surface {
    fn create(_parent: isize) -> Result<Self> {
        Ok(Self)
    }
    fn wid(&self) -> isize {
        0
    }
    fn set_position(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    fn set_z_order(&self, _top: bool) {}
    fn show(&self) {}
    fn hide(&self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Error,
    Stopped,
    Unknown,
}

pub struct MpvClient {
    ffi: Arc<MpvFfi>,
    handle: *mut c_void,
    app: AppHandle,
    snapshot: RwLock<PlayerSnapshot>,
    surface: Mutex<Option<Surface>>,
    end_callback: Mutex<Option<Arc<dyn Fn(EndReason) + Send + Sync>>>,
    last_progress: Mutex<Instant>,
    progress_tick: AtomicU64,
}

// mpv handles are internally synchronized; all calls are short-lived and we
// never retain borrowed mpv memory across threads.
unsafe impl Send for MpvClient {}
unsafe impl Sync for MpvClient {}

impl MpvClient {
    pub fn new(app: AppHandle, data_dir: &Path) -> Result<Arc<Self>> {
        let ffi = MpvFfi::load()?;
        let handle = unsafe { (ffi.create)() };
        if handle.is_null() {
            return Err(AppError::playback("Could not create the media engine."));
        }
        let code = unsafe { (ffi.initialize)(handle) };
        ffi.check(code)?;

        let snapshot = PlayerSnapshot {
            status: PlayerStatus::Stopped,
            current: None,
            position_ms: 0,
            duration_ms: 0,
            volume: 0.8,
            muted: false,
            speed: 1.0,
            shuffle: false,
            repeat: PlayMode::Off,
            fullscreen: false,
            audio_tracks: Vec::new(),
            subtitle_tracks: Vec::new(),
            video_tracks: Vec::new(),
            active_audio_track: -1,
            active_subtitle_track: -1,
            active_video_track: -1,
            video_width: 0,
            video_height: 0,
            error: None,
        };

        let client = Arc::new(Self {
            ffi,
            handle,
            app,
            snapshot: RwLock::new(snapshot),
            surface: Mutex::new(None),
            end_callback: Mutex::new(None),
            last_progress: Mutex::new(Instant::now()),
            progress_tick: AtomicU64::new(0),
        });

        // Engine defaults. Scripts/bindings are disabled so the app owns all UI.
        client.set_string("idle", "yes")?;
        client.set_string("force-window", "no")?;
        client.set_string("terminal", "no")?;
        client.set_string("msg-level", "all=error")?;
        client.set_string("vo", "gpu")?;
        client.set_string("hwdec", "auto-safe")?;
        client.set_string("ao", "wasapi")?;
        client.set_string("audio-display", "no")?;
        client.set_string("input-default-bindings", "no")?;
        client.set_string("input-vo-keyboard", "no")?;
        client.set_string("osc", "no")?;
        client.set_string("osd-level", "0")?;
        client.set_string("keep-open", "no")?;
        client.set_string("image-display-duration", "0")?;
        client.set_string("screenshot-format", "png")?;
        client.set_i64("volume", 80)?;
        // Enable Windows SMTC media controls (media buttons next to clock/Quick Settings)
        let _ = client.set_string("media-controls", "yes");
        // Network stream defaults: User-Agent for streaming servers/CDNs (like archive.org) & auto-reconnect
        let _ = client.set_string(
            "user-agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );
        let _ = client.set_string(
            "demuxer-lavf-o",
            "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5",
        );
        let _ = client.set_string(
            "stream-lavf-o",
            "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5",
        );
        let _ = client.set_string("network-timeout", "30");
        let _ = client.set_string("tls-verify", "no");

        let screenshots = data_dir.join("screenshots");
        let _ = std::fs::create_dir_all(&screenshots);
        client.set_string("screenshot-directory", &screenshots.to_string_lossy())?;

        // mpv requires each observed property to be requested in its native
        // format; a mismatched format makes the observation silently fail.
        let observed: [(&str, c_int); 9] = [
            ("pause", MPV_FORMAT_FLAG),
            ("volume", MPV_FORMAT_DOUBLE),
            ("mute", MPV_FORMAT_FLAG),
            ("speed", MPV_FORMAT_DOUBLE),
            ("duration", MPV_FORMAT_DOUBLE),
            ("aid", MPV_FORMAT_INT64),
            ("sid", MPV_FORMAT_INT64),
            ("fullscreen", MPV_FORMAT_FLAG),
            ("paused-for-cache", MPV_FORMAT_FLAG),
        ];
        for (idx, (property, format)) in observed.into_iter().enumerate() {
            let name = CString::new(property).unwrap();
            let code = unsafe {
                (client.ffi.observe_property)(client.handle, (idx + 1) as u64, name.as_ptr(), format)
            };
            if code < 0 {
                tracing::warn!("Could not observe mpv property {property}: {}", client.ffi.error_message(code));
            }
        }

        client.start_event_loop();
        Ok(client)
    }

    pub fn on_end<F>(&self, callback: F)
    where
        F: Fn(EndReason) + Send + Sync + 'static,
    {
        *self.end_callback.lock().unwrap() = Some(Arc::new(callback));
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        self.snapshot.read().unwrap().clone()
    }

    // --- Low level helpers -------------------------------------------------

    fn cstring(value: &str) -> Result<CString> {
        MpvFfi::cstring(value)
    }

    fn get_string(&self, name: &str) -> Option<String> {
        let name = Self::cstring(name).ok()?;
        let ptr = unsafe { (self.ffi.get_property_string)(self.handle, name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
        unsafe { (self.ffi.free)(ptr as *mut c_void) };
        Some(value)
    }

    fn get_i64_opt(&self, name: &str) -> Option<i64> {
        let name = Self::cstring(name).ok()?;
        let mut value: i64 = 0;
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_INT64,
                &mut value as *mut i64 as *mut c_void,
            )
        };
        if code >= 0 { Some(value) } else { None }
    }

    fn get_f64_opt(&self, name: &str) -> Option<f64> {
        let name = Self::cstring(name).ok()?;
        let mut value: f64 = 0.0;
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut value as *mut f64 as *mut c_void,
            )
        };
        if code >= 0 { Some(value) } else { None }
    }

    fn get_bool(&self, name: &str) -> bool {
        let name = match Self::cstring(name) {
            Ok(name) => name,
            Err(_) => return false,
        };
        let mut value: c_int = 0;
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut value as *mut c_int as *mut c_void,
            )
        };
        if code >= 0 {
            value != 0
        } else {
            false
        }
    }

    fn set_string(&self, name: &str, value: &str) -> Result<()> {
        let name = Self::cstring(name)?;
        let value = Self::cstring(value)?;
        let code = unsafe {
            (self.ffi.set_property_string)(self.handle, name.as_ptr(), value.as_ptr())
        };
        self.ffi.check(code)
    }

    fn set_i64(&self, name: &str, value: i64) -> Result<()> {
        let name = Self::cstring(name)?;
        let mut value = value;
        let code = unsafe {
            (self.ffi.set_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_INT64,
                &mut value as *mut i64 as *mut c_void,
            )
        };
        self.ffi.check(code)
    }

    fn set_f64(&self, name: &str, value: f64) -> Result<()> {
        let name = Self::cstring(name)?;
        let mut value = value;
        let code = unsafe {
            (self.ffi.set_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut value as *mut f64 as *mut c_void,
            )
        };
        self.ffi.check(code)
    }

    fn set_flag(&self, name: &str, value: bool) -> Result<()> {
        let name = Self::cstring(name)?;
        let mut value: c_int = i32::from(value);
        let code = unsafe {
            (self.ffi.set_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut value as *mut c_int as *mut c_void,
            )
        };
        self.ffi.check(code)
    }

    pub fn command(&self, args: &[&str]) -> Result<()> {
        let owned: Vec<CString> = args.iter().map(|a| Self::cstring(a)).collect::<Result<_>>()?;
        let mut pointers: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        pointers.push(std::ptr::null());
        let code = unsafe { (self.ffi.command)(self.handle, pointers.as_ptr()) };
        self.ffi.check(code)
    }

    // --- Playback control --------------------------------------------------

    pub fn load(&self, media: &MediaItem) -> Result<()> {
        let info = TrackInfo {
            id: media.id,
            path: media.path.clone(),
            title: media.title.clone(),
            artist: media.artist.clone(),
            album: media.album.clone(),
            kind: media.kind,
            thumbnail_path: media.thumbnail_path.clone(),
            duration_ms: media.duration_ms,
        };
        {
            let mut snapshot = self.snapshot.write().unwrap();
            snapshot.current = Some(info.clone());
            snapshot.status = PlayerStatus::Loading;
            snapshot.position_ms = 0;
            snapshot.duration_ms = media.duration_ms;
            snapshot.error = None;
        }

        if matches!(media.kind, crate::models::MediaKind::Video) {
            let parent = parent_hwnd(&self.app)?;
            let mut guard = self.surface.lock().unwrap();
            if guard.is_none() {
                *guard = Some(Surface::create(parent)?);
            }
            let wid = guard.as_ref().unwrap().wid() as i64;
            self.set_i64("wid", wid)?;
            guard.as_ref().unwrap().show();
            let _ = self.set_string("audio-display", "attachment");
            let _ = self.set_string("vid", "auto");
        } else {
            if let Some(surface) = self.surface.lock().unwrap().as_ref() {
                surface.hide();
            }
            let _ = self.set_i64("wid", 0);
            let _ = self.set_string("audio-display", "no");
            let _ = self.set_string("vid", "no");
        }

        let mut target_path = media.path.clone();
        if target_path.starts_with("http://") || target_path.starts_with("https://") {
            if target_path.starts_with("http://") && target_path.contains("archive.org") {
                target_path = format!("https://{}", &target_path[7..]);
            }
            if target_path.contains(' ') {
                target_path = target_path.replace(' ', "%20");
            }
        }
        let path = Self::cstring(&target_path)?;
        self.command(&["loadfile", &path.to_string_lossy(), "replace"])?;
        self.set_flag("pause", false)?;
        self.refresh_snapshot();
        self.emit_state();
        let _ = self.app.emit(events::PLAYER_FILE_LOADED, info);
        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<()> {
        self.command(&["cycle", "pause"])?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) -> Result<()> {
        self.set_flag("pause", paused)?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.command(&["stop"])?;
        {
            let mut snapshot = self.snapshot.write().unwrap();
            snapshot.status = PlayerStatus::Stopped;
            snapshot.current = None;
            snapshot.position_ms = 0;
            snapshot.duration_ms = 0;
        }
        if let Some(surface) = self.surface.lock().unwrap().as_ref() {
            surface.hide();
        }
        self.emit_state();
        Ok(())
    }

    pub fn seek_ms(&self, position_ms: i64) -> Result<()> {
        let status = self.snapshot.read().unwrap().status;
        if status == PlayerStatus::Stopped {
            return Ok(());
        }
        let seconds = (position_ms.max(0) as f64) / 1000.0;
        let value = format!("{seconds:.3}");
        let res = self.command(&["seek", &value, "absolute"]);
        if let Err(err) = res {
            tracing::warn!("mpv seek ignored or failed: {err}");
        }
        Ok(())
    }

    pub fn seek_relative(&self, delta_ms: i64) -> Result<()> {
        let status = self.snapshot.read().unwrap().status;
        if status == PlayerStatus::Stopped {
            return Ok(());
        }
        let seconds = delta_ms as f64 / 1000.0;
        let value = format!("{seconds:.3}");
        let res = self.command(&["seek", &value, "relative"]);
        if let Err(err) = res {
            tracing::warn!("mpv seek_relative ignored or failed: {err}");
        }
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) -> Result<()> {
        let clamped = volume.clamp(0.0, 1.0) * 100.0;
        self.set_f64("volume", clamped)?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn set_muted(&self, muted: bool) -> Result<()> {
        self.set_flag("mute", muted)?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        self.set_f64("speed", speed.clamp(0.1, 4.0))?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn set_shuffle(&self, shuffle: bool) -> Result<()> {
        self.snapshot.write().unwrap().shuffle = shuffle;
        self.emit_state();
        Ok(())
    }

    pub fn set_repeat(&self, repeat: PlayMode) -> Result<()> {
        self.snapshot.write().unwrap().repeat = repeat;
        self.emit_state();
        Ok(())
    }

    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        self.set_flag("fullscreen", fullscreen)?;
        self.refresh_snapshot();
        self.emit_state();
        Ok(())
    }

    pub fn set_video_track(&self, id: i64) -> Result<()> {
        if id < 0 {
            self.set_string("vid", "auto")?;
        } else {
            self.set_i64("vid", id)?;
        }
        self.emit_tracks();
        Ok(())
    }

    pub fn set_audio_track(&self, id: i64) -> Result<()> {
        self.set_i64("aid", id)?;
        self.emit_tracks();
        Ok(())
    }

    pub fn set_subtitle_track(&self, id: i64) -> Result<()> {
        self.set_i64("sid", id)?;
        self.emit_tracks();
        Ok(())
    }

    pub fn add_subtitle(&self, path: &str) -> Result<()> {
        let path = Self::cstring(path)?;
        self.command(&["sub-add", &path.to_string_lossy(), "select"])?;
        self.emit_tracks();
        Ok(())
    }

    pub fn set_subtitle_delay(&self, delay_ms: i64) -> Result<()> {
        let seconds = delay_ms as f64 / 1000.0;
        self.set_f64("sub-delay", seconds)
    }

    pub fn frame_step(&self) -> Result<()> {
        self.command(&["frame-step"])
    }

    pub fn set_aspect_ratio(&self, ratio: &str) -> Result<()> {
        self.set_string("video-aspect-override", ratio)
    }

    /// Apply (or, with an empty value, clear) a runtime mpv string property.
    /// Used for `glsl-shaders` when the AI Enhance toggle is switched.
    pub fn set_string_property(&self, name: &str, value: &str) -> Result<()> {
        self.set_string(name, value)
    }

    /// Set a numeric mpv property (brightness, contrast, saturation, gamma, …).
    pub fn set_i64_property(&self, name: &str, value: i64) -> Result<()> {
        self.set_i64(name, value)
    }

    pub fn get_string_property(&self, name: &str) -> Option<String> {
        self.get_string(name)
    }

    /// Capture the current video frame to `path` in the requested image format.
    /// The user's screenshot-format preference is restored even if capture fails.
    pub fn capture_frame(&self, path: &std::path::Path, format: &str) -> Result<()> {
        let previous = self.get_string("screenshot-format").unwrap_or_else(|| "png".into());
        self.set_string("screenshot-format", format)?;
        let result = self.command(&["screenshot-to-file", &path.to_string_lossy(), "video"]);
        let _ = self.set_string("screenshot-format", &previous);
        result
    }

    pub fn screenshot(&self) -> Result<String> {
        let directory = self.get_string("screenshot-directory").unwrap_or_default();
        let name = format!(
            "slideup-{}.png",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        self.command(&["screenshot-to-file", &format!("{directory}\\{name}"), "video"])?;
        Ok(format!("{directory}\\{name}"))
    }

    pub fn sync_video_rect(&self, x: i64, y: i64, width: i64, height: i64) -> Result<()> {
        if let Some(surface) = self.surface.lock().unwrap().as_ref() {
            surface.set_position(x as i32, y as i32, width as i32, height as i32);
        }
        Ok(())
    }

    /// Show or hide the native video HWND. Called when the video surface is
    /// mounted/unmounted or the user navigates away from the player view.
    pub fn set_surface_visible(&self, visible: bool) -> Result<()> {
        if let Some(surface) = self.surface.lock().unwrap().as_ref() {
            if visible {
                surface.show();
            } else {
                surface.hide();
            }
        }
        Ok(())
    }

    /// Move the native video HWND above or below the webview. The frontend
    /// calls this on window focus changes to trade DOM overlays for cheaper
    /// compositing while the player is in the background.
    pub fn set_video_layer(&self, top: bool) -> Result<()> {
        if let Some(surface) = self.surface.lock().unwrap().as_ref() {
            surface.set_z_order(top);
        }
        Ok(())
    }

    pub fn apply_settings(&self, settings: &AppSettings, previous: Option<&AppSettings>) -> Result<()> {
        let should_apply_all = previous.is_none();

        if should_apply_all || previous.map(|p| (p.volume - settings.volume).abs() > 0.001).unwrap_or(false) {
            self.set_volume(settings.volume)?;
        }

        if should_apply_all || previous.map(|p| p.muted != settings.muted).unwrap_or(false) {
            self.set_muted(settings.muted)?;
        }

        if should_apply_all || previous.map(|p| (p.playback_speed - settings.playback_speed).abs() > 0.001).unwrap_or(false) {
            self.set_speed(settings.playback_speed)?;
        }

        if should_apply_all || previous.map(|p| p.hardware_acceleration != settings.hardware_acceleration).unwrap_or(false) {
            self.set_string(
                "hwdec",
                if settings.hardware_acceleration {
                    "auto-safe"
                } else {
                    "no"
                },
            )?;
        }

        if should_apply_all || previous.map(|p| p.audio_output != settings.audio_output).unwrap_or(false) {
            let ao = match settings.audio_output.as_str() {
                "auto" | "" => "wasapi",
                other => other,
            };
            if let Err(err) = self.set_string("ao", ao) {
                tracing::warn!("Failed to set audio driver '{ao}': {err}; falling back to wasapi");
                let _ = self.set_string("ao", "wasapi");
            }
        }

        if should_apply_all || previous.map(|p| p.audio_device != settings.audio_device).unwrap_or(false) {
            let device = match settings.audio_device.as_str() {
                "" => "auto",
                other => other,
            };
            if let Err(err) = self.set_string("audio-device", device) {
                tracing::warn!("Failed to set audio device '{device}': {err}; falling back to auto");
                let _ = self.set_string("audio-device", "auto");
            }
        }

        if should_apply_all || previous.map(|p| p.subtitle_font != settings.subtitle_font).unwrap_or(false) {
            self.set_string("sub-font", &settings.subtitle_font)?;
        }

        if should_apply_all || previous.map(|p| p.subtitle_size != settings.subtitle_size).unwrap_or(false) {
            self.set_i64("sub-font-size", settings.subtitle_size)?;
        }

        if should_apply_all || previous.map(|p| p.subtitle_color != settings.subtitle_color).unwrap_or(false) {
            self.set_string("sub-color", &settings.subtitle_color)?;
        }

        if should_apply_all || previous.map(|p| p.subtitle_border_size != settings.subtitle_border_size).unwrap_or(false) {
            self.set_i64("sub-border-size", settings.subtitle_border_size)?;
        }

        if should_apply_all || previous.map(|p| p.subtitle_margin != settings.subtitle_margin).unwrap_or(false) {
            self.set_i64("sub-margin-y", settings.subtitle_margin)?;
        }

        if should_apply_all || previous.map(|p| p.subtitle_delay_ms != settings.subtitle_delay_ms).unwrap_or(false) {
            self.set_f64("sub-delay", settings.subtitle_delay_ms as f64 / 1000.0)?;
        }

        if should_apply_all
            || previous
                .map(|p| {
                    p.video_brightness != settings.video_brightness
                        || p.video_contrast != settings.video_contrast
                        || p.video_saturation != settings.video_saturation
                        || p.video_gamma != settings.video_gamma
                })
                .unwrap_or(false)
        {
            // These properties only exist while a video track is active, so a
            // failed set must not abort the rest of settings application.
            let _ = self.set_i64("brightness", settings.video_brightness);
            let _ = self.set_i64("contrast", settings.video_contrast);
            let _ = self.set_i64("saturation", settings.video_saturation);
            let _ = self.set_i64("gamma", settings.video_gamma);
        }

        Ok(())
    }

    /// Enumerates the audio output devices currently known to mpv.
    pub fn audio_devices(&self) -> Vec<AudioDevice> {
        let name = match Self::cstring("audio-device-list") {
            Ok(name) => name,
            Err(_) => return Vec::new(),
        };
        let mut node: MpvNode = unsafe { std::mem::zeroed() };
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_NODE,
                &mut node as *mut MpvNode as *mut c_void,
            )
        };
        if code < 0 || node.format != MPV_FORMAT_NODE_ARRAY {
            return Vec::new();
        }
        let mut devices = Vec::new();
        unsafe {
            let list = &*node.value.list;
            for index in 0..list.num as usize {
                let item = &*list.values.add(index);
                if item.format != MPV_FORMAT_NODE_MAP {
                    continue;
                }
                let device_name = node_string(item, "name").unwrap_or_default();
                if device_name.is_empty() {
                    continue;
                }
                devices.push(AudioDevice {
                    name: device_name,
                    description: node_string(item, "description")
                        .unwrap_or_else(|| "Unknown device".into()),
                });
            }
            (self.ffi.free_node_contents)(&mut node);
        }
        devices
    }

    // --- State / events ----------------------------------------------------

    fn refresh_snapshot(&self) {
        let mut snapshot = self.snapshot.write().unwrap();
        snapshot.position_ms = ((self.get_f64_opt("time-pos").unwrap_or(0.0)) * 1000.0) as i64;
        snapshot.duration_ms = ((self.get_f64_opt("duration").unwrap_or(0.0)) * 1000.0) as i64;
        snapshot.volume = (self.get_f64_opt("volume").unwrap_or(0.0) / 100.0).clamp(0.0, 1.0);
        snapshot.muted = self.get_bool("mute");
        snapshot.speed = self.get_f64_opt("speed").unwrap_or(1.0);
        snapshot.fullscreen = self.get_bool("fullscreen");
        snapshot.active_audio_track = self.get_i64_opt("aid").unwrap_or(-1);
        snapshot.active_subtitle_track = self.get_i64_opt("sid").unwrap_or(-1);
        snapshot.video_width = self.get_i64_opt("video-params/w").unwrap_or(0);
        snapshot.video_height = self.get_i64_opt("video-params/h").unwrap_or(0);
        if snapshot.current.is_some() {
            let paused_for_cache = self.get_bool("paused-for-cache");
            snapshot.status = if paused_for_cache {
                PlayerStatus::Loading
            } else if self.get_bool("pause") {
                PlayerStatus::Paused
            } else {
                PlayerStatus::Playing
            };
        }
    }

    fn emit_state(&self) {
        let snapshot = self.snapshot();
        let _ = self.app.emit(events::PLAYER_STATE, snapshot);
    }

    fn emit_tracks(&self) {
        let (video, audio, subtitles, vid, aid, sid) = {
            let (video, audio, subtitles) = self.tracks();
            (
                video,
                audio,
                subtitles,
                self.get_i64_opt("vid").unwrap_or(-1),
                self.get_i64_opt("aid").unwrap_or(-1),
                self.get_i64_opt("sid").unwrap_or(-1),
            )
        };
        {
            let mut snapshot = self.snapshot.write().unwrap();
            snapshot.video_tracks = video.clone();
            snapshot.audio_tracks = audio.clone();
            snapshot.subtitle_tracks = subtitles.clone();
            snapshot.active_video_track = vid;
            snapshot.active_audio_track = aid;
            snapshot.active_subtitle_track = sid;
        }
        let _ = self.app.emit(
            events::PLAYER_TRACKS,
            serde_json::json!({
                "videoTracks": video,
                "audioTracks": audio,
                "subtitleTracks": subtitles,
                "activeVideoTrack": vid,
                "activeAudioTrack": aid,
                "activeSubtitleTrack": sid,
            }),
        );
    }

    fn tracks(&self) -> (Vec<MediaTrack>, Vec<MediaTrack>, Vec<MediaTrack>) {
        let name = match Self::cstring("track-list") {
            Ok(name) => name,
            Err(_) => return (Vec::new(), Vec::new(), Vec::new()),
        };
        let mut node: MpvNode = unsafe { std::mem::zeroed() };
        let code = unsafe {
            (self.ffi.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_NODE,
                &mut node as *mut MpvNode as *mut c_void,
            )
        };
        if code < 0 || node.format != MPV_FORMAT_NODE_ARRAY {
            return (Vec::new(), Vec::new(), Vec::new());
        }

        let mut video = Vec::new();
        let mut audio = Vec::new();
        let mut subtitles = Vec::new();
        unsafe {
            let list = &*node.value.list;
            for index in 0..list.num as usize {
                let item = &*list.values.add(index);
                if item.format != MPV_FORMAT_NODE_MAP {
                    continue;
                }
                let kind = node_string(item, "type").unwrap_or_default();
                if kind != "video" && kind != "audio" && kind != "sub" {
                    continue;
                }
                let track = MediaTrack {
                    id: node_i64(item, "id").unwrap_or(-1),
                    name: if kind == "video" {
                        let w = node_i64(item, "demux-w").unwrap_or(0);
                        let h = node_i64(item, "demux-h").unwrap_or(0);
                        let fps = node_i64(item, "demux-fps").unwrap_or(0);
                        let title = node_string(item, "title");
                        if let Some(t) = title {
                            if !t.trim().is_empty() {
                                t
                            } else if h > 0 {
                                if fps > 30 {
                                    format!("{}p{}", h, fps)
                                } else {
                                    format!("{}p", h)
                                }
                            } else {
                                format!("Track {}", index + 1)
                            }
                        } else if h > 0 {
                            if fps > 30 {
                                format!("{}p{}", h, fps)
                            } else if w > 0 {
                                format!("{}p ({}x{})", h, w, h)
                            } else {
                                format!("{}p", h)
                            }
                        } else {
                            format!("Track {}", index + 1)
                        }
                    } else {
                        node_string(item, "title")
                            .or_else(|| node_string(item, "lang"))
                            .unwrap_or_else(|| format!("Track {}", index + 1))
                    },
                    language: node_string(item, "lang"),
                    selected: node_bool(item, "selected").unwrap_or(false),
                };
                if kind == "video" {
                    video.push(track);
                } else if kind == "audio" {
                    audio.push(track);
                } else {
                    subtitles.push(track);
                }
            }
            (self.ffi.free_node_contents)(&mut node);
        }
        (video, audio, subtitles)
    }

    fn start_event_loop(self: &Arc<Self>) {
        let client = Arc::clone(self);
        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || client.event_loop())
            .expect("failed to start mpv event thread");
    }

    fn event_loop(self: Arc<Self>) {
        loop {
            let event = unsafe { (self.ffi.wait_event)(self.handle, 0.1) };
            if event.is_null() {
                continue;
            }
            let event_ref = unsafe { *event };
            match event_ref.event_id {
                MPV_EVENT_NONE => {}
                MPV_EVENT_SHUTDOWN => {
                    tracing::warn!("mpv engine shut down");
                    break;
                }
                MPV_EVENT_FILE_LOADED => {
                    self.refresh_snapshot();
                    self.emit_state();
                    self.emit_tracks();
                }
                MPV_EVENT_END_FILE => {
                    let info = unsafe { &*(event_ref.data as *const MpvEventEndFile) };
                    let reason = match info.reason {
                        END_REASON_EOF => EndReason::Eof,
                        END_REASON_ERROR => EndReason::Error,
                        _ if info.error < 0 => EndReason::Error,
                        _ => EndReason::Stopped,
                    };
                    if info.error == -14 {
                        tracing::warn!("Audio output initialization failed; falling back to default wasapi output");
                        let _ = self.set_string("ao", "wasapi");
                        let _ = self.set_string("audio-device", "auto");
                    }
                    let repeat = self.snapshot.read().unwrap().repeat;
                    if reason == EndReason::Eof && repeat == PlayMode::One {
                        let _ = self.seek_ms(0);
                        let _ = self.set_paused(false);
                    } else {
                        self.snapshot.write().unwrap().position_ms = 0;
                        let _ = self.app.emit(events::PLAYER_ENDED, serde_json::json!({
                            "reason": match reason {
                                EndReason::Eof => "eof",
                                EndReason::Error => "error",
                                EndReason::Stopped => "stopped",
                                EndReason::Unknown => "unknown",
                            }
                        }));
                        if let Some(callback) = self.end_callback.lock().unwrap().as_ref() {
                            callback(reason);
                        }
                    }
                }
                MPV_EVENT_TRACKS_CHANGED => self.emit_tracks(),
                MPV_EVENT_PAUSE | MPV_EVENT_UNPAUSE => {
                    self.refresh_snapshot();
                    self.emit_state();
                }
                MPV_EVENT_PROPERTY_CHANGE => {
                    let property = unsafe { &*(event_ref.data as *const MpvEventProperty) };
                    if !property.name.is_null() {
                        let name = unsafe { CStr::from_ptr(property.name) }
                            .to_string_lossy()
                            .into_owned();
                        if matches!(
                            name.as_str(),
                            "pause" | "volume" | "mute" | "speed" | "duration" | "aid"
                                | "sid" | "fullscreen" | "paused-for-cache"
                        ) {
                            self.refresh_snapshot();
                            self.emit_state();
                        }
                    }
                }
                _ => {}
            }

            // Progress updates are polled (not observed) to avoid per-frame events.
            let should_emit = self.last_progress.lock().unwrap().elapsed().as_millis() >= 200;
            if should_emit {
                *self.last_progress.lock().unwrap() = Instant::now();
                let snapshot = self.snapshot.read().unwrap();
                if snapshot.status == PlayerStatus::Playing {
                    let progress = crate::models::ProgressSnapshot {
                        position_ms: ((self.get_f64_opt("time-pos").unwrap_or(0.0)) * 1000.0) as i64,
                        duration_ms: ((self.get_f64_opt("duration").unwrap_or(0.0)) * 1000.0) as i64,
                        status: PlayerStatus::Playing,
                    };
                    drop(snapshot);
                    let _ = self.app.emit(events::PLAYER_PROGRESS, progress);
                    self.progress_tick.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
}

impl Drop for MpvClient {
    fn drop(&mut self) {
        unsafe {
            (self.ffi.destroy)(self.handle);
        }
    }
}

fn parent_hwnd(app: &AppHandle) -> Result<isize> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::native("The main window is not available."))?;
    #[cfg(windows)]
    {
        let hwnd = window
            .hwnd()
            .map_err(|err| AppError::native(format!("Could not read the window handle: {err}")))?;
        // HWND is a pointer-sized value in every supported windows-sys version.
        let raw: isize = unsafe { std::mem::transmute_copy(&hwnd) };
        Ok(raw)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Ok(0)
    }
}

fn node_field<'a>(node: &'a MpvNode, key: &str) -> Option<&'a MpvNode> {
    if node.format != MPV_FORMAT_NODE_MAP {
        return None;
    }
    unsafe {
        let list = &*node.value.list;
        if list.keys.is_null() || list.values.is_null() {
            return None;
        }
        for index in 0..list.num as usize {
            let key_ptr = *list.keys.add(index);
            if key_ptr.is_null() {
                continue;
            }
            if CStr::from_ptr(key_ptr).to_bytes() == key.as_bytes() {
                return Some(&*list.values.add(index));
            }
        }
        None
    }
}

fn node_string(node: &MpvNode, key: &str) -> Option<String> {
    unsafe {
        let field = node_field(node, key)?;
        if field.format != MPV_FORMAT_STRING || field.value.string.is_null() {
            return None;
        }
        Some(CStr::from_ptr(field.value.string).to_string_lossy().into_owned())
    }
}

fn node_i64(node: &MpvNode, key: &str) -> Option<i64> {
    unsafe {
        let field = node_field(node, key)?;
        if field.format == MPV_FORMAT_INT64 {
            Some(field.value.int64)
        } else {
            None
        }
    }
}

fn node_bool(node: &MpvNode, key: &str) -> Option<bool> {
    unsafe {
        let field = node_field(node, key)?;
        if field.format == MPV_FORMAT_FLAG {
            Some(field.value.flag != 0)
        } else {
            None
        }
    }
}
