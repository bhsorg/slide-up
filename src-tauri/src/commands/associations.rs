//! Runtime file association management (Windows per-user registry + Shell COM).
//!
//! Built-in audio/video formats are shared by every installation and cannot be
//! removed individually. Extensions added by the user live in `AppSettings`
//! and can be removed again — built-in entries are never stored there.
//!
//! Modern Windows stores the *effective* default in
//! `Explorer\FileExts\<ext>\UserChoice`, which cannot be written directly
//! (it is hash-protected). We therefore register the app through the official
//! `RegisteredApplications` / `Capabilities` mechanism so it appears in
//! Explorer's "Open with" list and in Settings → Default Apps, and we ask the
//! Shell to set the default through `IApplicationAssociationRegistration`.
//! When Windows refuses a silent default change (common on Windows 10/11),
//! the UI can open the Default Apps page for this app so the user can confirm.

use crate::error::{AppError, Result};
use crate::models::{CustomExtension, FileAssociations, FileExtension};
use crate::state::AppState;
use tauri::State;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    ApplicationAssociationRegistration, IApplicationAssociationRegistration, AL_EFFECTIVE,
    AT_FILEEXTENSION, SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHChangeNotify,
};

const PROG_ID: &str = "SlideUp.Media";
const APP_EXE: &str = "slide-up.exe";
/// Key name under HKCU\Software\RegisteredApplications.
const APP_REG_NAME: &str = "Slide Up";
const CAPABILITIES_KEY: &str = "Software\\Slide Up\\Capabilities";
/// CREATE_NO_WINDOW — keeps reg.exe from flashing a console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "m4b", "opus", "wma", "aiff", "aif",
    "alac", "ape", "wv", "mp2", "mp1", "caf", "amr", "mka", "dts", "ac3", "eac3", "mid", "midi",
];

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "avi", "mov", "webm", "wmv", "flv", "mpg", "mpeg", "m2v", "ts",
    "m2ts", "mts", "3gp", "3g2", "ogv", "vob", "divx", "rmvb", "rm", "asf", "hevc", "h264", "f4v",
];

fn normalize_extension(raw: &str) -> Option<String> {
    let value = raw.trim().trim_start_matches('.').to_lowercase();
    if value.is_empty() || value.len() > 16 {
        return None;
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '_')
    {
        return None;
    }
    Some(value)
}

fn is_built_in(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext) || VIDEO_EXTENSIONS.contains(&ext)
}

fn exe_path() -> Result<String> {
    let exe = std::env::current_exe()
        .map_err(|err| AppError::native(format!("Could not locate the application executable: {err}")))?;
    Ok(exe.to_string_lossy().into_owned())
}

/// Escape a value for use inside a .reg file string.
fn escape_reg(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Format a raw value as a quoted .reg string literal.
///
/// `reg.exe` exits successfully even when a value line is malformed (the
/// value is silently skipped), so every value must be quoted and escaped
/// exactly once.
fn reg_string(value: &str) -> String {
    format!("\"{}\"", escape_reg(value))
}

fn apply_reg(content: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let path = std::env::temp_dir().join("slideup-associations.reg");
        std::fs::write(&path, content.as_bytes())
            .map_err(|err| AppError::native(format!("Could not prepare the association file: {err}")))?;

        let mut command = Command::new("reg.exe");
        command.arg("import").arg(&path);
        command.creation_flags(CREATE_NO_WINDOW);
        let output = command
            .output()
            .map_err(|err| AppError::native(format!("Could not start reg.exe: {err}")))?;
        let _ = std::fs::remove_file(&path);

        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(AppError::native(if message.is_empty() {
                "Windows refused to update the file associations.".into()
            } else {
                format!("Could not update file associations: {message}")
            }));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = content;
        Err(AppError::native(
            "File association registration is only available on Windows.",
        ))
    }
}

/// Ask Explorer to refresh cached association information immediately.
fn notify_shell() {
    #[cfg(windows)]
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, None, None);
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Run a closure against the Shell association-registration COM object.
/// Returns `None` when COM is unavailable; callers fall back to the registry.
#[cfg(windows)]
fn with_associations<T>(
    action: impl FnOnce(&IApplicationAssociationRegistration) -> windows::core::Result<T>,
) -> Option<T> {
    unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| {
            let registration: IApplicationAssociationRegistration = CoCreateInstance(
                &ApplicationAssociationRegistration,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .ok()?;
            action(&registration).ok()
        })();
        if initialized {
            CoUninitialize();
        }
        result
    }
}

/// Ask the Shell to make Slide Up the effective default for these extensions.
/// Windows may refuse (E_ACCESSDENIED) when a user choice already exists;
/// those failures are non-fatal because the app is still registered for
/// "Open with" and the user can confirm in Default Apps settings.
fn apply_shell_defaults(exts: &[(String, String)]) {
    #[cfg(windows)]
    {
        let _ = with_associations(|registration| {
            for (ext, _) in exts {
                let app = wide(APP_REG_NAME);
                let extension = wide(ext);
                if let Err(error) = unsafe {
                    registration.SetAppAsDefault(
                        windows::core::PCWSTR(app.as_ptr()),
                        windows::core::PCWSTR(extension.as_ptr()),
                        AT_FILEEXTENSION,
                    )
                } {
                    tracing::debug!("Default for .{ext} needs manual confirmation: {error}");
                }
            }
            Ok(())
        });
    }
    #[cfg(not(windows))]
    {
        let _ = exts;
    }
}

fn registry_says_registered(ext: &str) -> bool {
    #[cfg(windows)]
    {
        let key = format!("HKCU\\Software\\Classes\\.{ext}");
        let output = Command::new("reg.exe")
            .args(["query", &key, "/ve"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        matches!(output, Ok(result) if result.status.success()
            && String::from_utf8_lossy(&result.stdout).contains(PROG_ID))
    }
    #[cfg(not(windows))]
    {
        let _ = ext;
        false
    }
}

fn is_registered(ext: &str) -> bool {
    #[cfg(windows)]
    {
        // The Shell knows about the hash-protected UserChoice default, so it
        // is the most accurate source. Fall back to our own registration keys
        // when Windows blocks a silent default change — the app is still
        // associated and available in "Open with".
        if let Some(is_default) = with_associations(|registration| {
            let app = wide(APP_REG_NAME);
            let extension = wide(ext);
            unsafe {
                registration.QueryAppIsDefault(
                    windows::core::PCWSTR(extension.as_ptr()),
                    AT_FILEEXTENSION,
                    AL_EFFECTIVE,
                    windows::core::PCWSTR(app.as_ptr()),
                )
            }
        }) {
            if is_default.as_bool() {
                return true;
            }
        }
    }
    registry_says_registered(ext)
}

fn build_register_content(exts: &[(String, String)], exe: &str) -> String {
    let mut content = String::from("Windows Registry Editor Version 5.00\n\n");

    for (ext, _) in exts {
        content.push_str(&format!(
            "[HKEY_CURRENT_USER\\Software\\Classes\\.{ext}]\n@=\"{PROG_ID}\"\n\n"
        ));
        content.push_str(&format!(
            "[HKEY_CURRENT_USER\\Software\\Classes\\.{ext}\\OpenWithProgids]\n\"{PROG_ID}\"=\"\"\n\n"
        ));
    }

    let icon = format!("\"{exe}\",0");
    let command = format!("\"{exe}\" \"%1\"");
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\Classes\\{PROG_ID}]\n@=\"Slide Up Media File\"\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\Classes\\{PROG_ID}\\DefaultIcon]\n@={}\n\n",
        reg_string(&icon)
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\Classes\\{PROG_ID}\\shell\\open]\n@=\"Open with Slide Up\"\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\Classes\\{PROG_ID}\\shell\\open\\command]\n@={}\n\n",
        reg_string(&command)
    ));

    let app_key = format!("HKEY_CURRENT_USER\\Software\\Classes\\Applications\\{APP_EXE}");
    content.push_str(&format!("[{app_key}]\n@=\"Slide Up\"\n\n"));
    content.push_str(&format!(
        "[{app_key}\\shell\\open\\command]\n@={}\n\n",
        reg_string(&command)
    ));
    content.push_str(&format!("[{app_key}\\SupportedTypes]\n"));
    for (ext, _) in exts {
        content.push_str(&format!("\".{ext}\"=\"\"\n"));
    }
    content.push('\n');

    // Register the application itself so Windows lists it in "Open with" and
    // in Settings → Default Apps (the UserChoice-aware association path).
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\RegisteredApplications]\n\"{APP_REG_NAME}\"=\"{}\"\n\n",
        escape_reg(CAPABILITIES_KEY)
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\{CAPABILITIES_KEY}]\n\"ApplicationName\"=\"Slide Up\"\n\"ApplicationDescription\"=\"Play audio and video files with Slide Up.\"\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\{CAPABILITIES_KEY}\\FileAssociations]\n"
    ));
    for (ext, _) in exts {
        content.push_str(&format!("\".{ext}\"=\"{PROG_ID}\"\n"));
    }
    content.push('\n');
    content
}

fn build_unregister_content(exts: &[(String, String)]) -> String {
    let mut content = String::from("Windows Registry Editor Version 5.00\n\n");
    for (ext, _) in exts {
        content.push_str(&format!(
            "[-HKEY_CURRENT_USER\\Software\\Classes\\.{ext}]\n\n"
        ));
    }
    content.push_str(&format!(
        "[-HKEY_CURRENT_USER\\Software\\Classes\\{PROG_ID}]\n\n"
    ));
    content.push_str(&format!(
        "[-HKEY_CURRENT_USER\\Software\\Classes\\Applications\\{APP_EXE}]\n\n"
    ));
    content.push_str(&format!(
        "[-HKEY_CURRENT_USER\\{CAPABILITIES_KEY}]\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\RegisteredApplications]\n\"{APP_REG_NAME}\"=-\n\n"
    ));
    content
}

fn build_remove_custom_content(ext: &str) -> String {
    let mut content = String::from("Windows Registry Editor Version 5.00\n\n");
    content.push_str(&format!(
        "[-HKEY_CURRENT_USER\\Software\\Classes\\.{ext}]\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\Software\\Classes\\Applications\\{APP_EXE}\\SupportedTypes]\n\".{ext}\"=-\n\n"
    ));
    content.push_str(&format!(
        "[HKEY_CURRENT_USER\\{CAPABILITIES_KEY}\\FileAssociations]\n\".{ext}\"=-\n\n"
    ));
    content
}

fn all_extensions(state: &AppState) -> Vec<(String, String)> {
    let mut exts: Vec<(String, String)> = AUDIO_EXTENSIONS
        .iter()
        .map(|ext| ((*ext).to_string(), "audio".to_string()))
        .collect();
    exts.extend(
        VIDEO_EXTENSIONS
            .iter()
            .map(|ext| ((*ext).to_string(), "video".to_string())),
    );
    let custom = state.settings.lock().unwrap().custom_extensions.clone();
    exts.extend(custom.into_iter().map(|item| (item.ext, item.kind)));
    exts
}

fn prog_id_has_open_command() -> bool {
    #[cfg(windows)]
    {
        let key = format!("HKCU\\Software\\Classes\\{PROG_ID}\\shell\\open\\command");
        let output = Command::new("reg.exe")
            .args(["query", &key, "/ve"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        matches!(output, Ok(result) if result.status.success()
            && String::from_utf8_lossy(&result.stdout).contains(APP_EXE))
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn collect(state: &AppState) -> FileAssociations {
    let custom = state.settings.lock().unwrap().custom_extensions.clone();
    // A `.ext` default that points at our ProgID is not enough: Windows can
    // only launch the app when the ProgID has a usable open command.
    let usable = prog_id_has_open_command();
    let map = |ext: &str, kind: &str, custom: bool| FileExtension {
        ext: ext.to_string(),
        kind: kind.to_string(),
        registered: usable && is_registered(ext),
        custom,
    };

    let audio: Vec<FileExtension> = AUDIO_EXTENSIONS
        .iter()
        .map(|ext| map(ext, "audio", false))
        .collect();
    let video: Vec<FileExtension> = VIDEO_EXTENSIONS
        .iter()
        .map(|ext| map(ext, "video", false))
        .collect();
    let custom_items: Vec<FileExtension> = custom
        .iter()
        .map(|item| map(&item.ext, &item.kind, true))
        .collect();

    let registered =
        audio.iter().all(|item| item.registered) && video.iter().all(|item| item.registered);

    FileAssociations {
        registered,
        audio,
        video,
        custom: custom_items,
    }
}

#[tauri::command]
pub fn get_file_associations(state: State<'_, AppState>) -> FileAssociations {
    collect(&state)
}

#[tauri::command]
pub fn register_file_associations(state: State<'_, AppState>) -> Result<FileAssociations> {
    let exe = exe_path()?;
    let exts = all_extensions(&state);
    apply_reg(&build_register_content(&exts, &exe))?;
    apply_shell_defaults(&exts);
    notify_shell();
    Ok(collect(&state))
}

#[tauri::command]
pub fn unregister_file_associations(state: State<'_, AppState>) -> Result<FileAssociations> {
    let exts = all_extensions(&state);
    apply_reg(&build_unregister_content(&exts))?;
    notify_shell();
    Ok(collect(&state))
}

#[tauri::command]
pub fn add_custom_extension(
    state: State<'_, AppState>,
    ext: String,
    kind: String,
) -> Result<FileAssociations> {
    let ext = normalize_extension(&ext).ok_or_else(|| {
        AppError::native("Use 1–16 letters or numbers (a leading dot is allowed).")
    })?;
    let kind = match kind.as_str() {
        "audio" => "audio",
        "video" => "video",
        _ => return Err(AppError::native("Choose audio or video for this extension.")),
    };
    if is_built_in(&ext) {
        return Err(AppError::native(
            "This format is already part of the built-in list.",
        ));
    }

    let mut settings = state.settings.lock().unwrap().clone();
    if settings
        .custom_extensions
        .iter()
        .any(|item| item.ext == ext)
    {
        return Err(AppError::native("This extension is already in your custom list."));
    }
    settings.custom_extensions.push(CustomExtension {
        ext: ext.clone(),
        kind: kind.to_string(),
    });
    state.save_settings(&settings)?;
    drop(settings);

    // Keep an existing registration up to date without a second full apply.
    if collect(&state).registered {
        let exe = exe_path()?;
        let added = vec![(ext, kind.to_string())];
        apply_reg(&build_register_content(&added, &exe))?;
        apply_shell_defaults(&added);
        notify_shell();
    }
    Ok(collect(&state))
}

#[tauri::command]
pub fn remove_custom_extension(
    state: State<'_, AppState>,
    ext: String,
) -> Result<FileAssociations> {
    let ext = normalize_extension(&ext)
        .ok_or_else(|| AppError::native("That is not a valid extension."))?;

    let mut settings = state.settings.lock().unwrap().clone();
    let before = settings.custom_extensions.len();
    settings.custom_extensions.retain(|item| item.ext != ext);
    if settings.custom_extensions.len() == before {
        return Err(AppError::native("Only extensions you added can be removed."));
    }
    state.save_settings(&settings)?;
    drop(settings);

    apply_reg(&build_remove_custom_content(&ext))?;
    notify_shell();
    Ok(collect(&state))
}

/// Open Windows Settings on the Default Apps page for Slide Up.
///
/// The legacy `IApplicationAssociationRegistrationUI` COM class (`sud.dll`)
/// is deprecated and returns `ERROR_NO_ASSOCIATION` (0x80070483) on current
/// Windows builds, so use the documented `ms-settings:` URI instead.
#[tauri::command]
pub fn open_default_apps_settings() -> Result<()> {
    #[cfg(windows)]
    {
        tauri_plugin_opener::open_url("ms-settings:defaultapps", None::<&str>)
            .map_err(|error| AppError::native(format!("Could not open Default Apps: {error}")))
    }
    #[cfg(not(windows))]
    {
        Err(AppError::native(
            "Default Apps settings are only available on Windows.",
        ))
    }
}
