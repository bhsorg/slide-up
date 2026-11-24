// FFI bindings expose the complete libmpv surface, and several repository
// helpers are intentionally kept for future settings/library features.
#![allow(dead_code)]

mod commands;
mod db;
mod error;
mod events;
mod metadata;
mod model_manager;
mod onnx_caption_engine;
mod models;
mod mpv;
mod state;
mod upscaler_manager;
mod widget;
#[cfg(windows)]
mod taskbar;

use db::schema;
use db::Repository;
use models::{AppSettings, PlaybackSession};
use onnx_caption_engine::CaptionDevice;
use state::AppState;
use std::fs;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

fn started_from_autostart() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--from-autostart")
}

/// Media files passed to the process, ignoring the executable and flags.
fn media_paths_from_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .collect()
}

fn media_available(media: &models::MediaItem) -> bool {
    let path = &media.path;
    path.starts_with("http://")
        || path.starts_with("https://")
        || std::path::Path::new(path).is_file()
}

/// Pick the first available item, starting at the preferred index.
fn restorable_queue(
    media: Vec<models::MediaItem>,
    preferred: usize,
    name: String,
) -> Option<(Vec<models::MediaItem>, usize, String)> {
    if media.is_empty() {
        return None;
    }
    let preferred = preferred.min(media.len() - 1);
    let mut order: Vec<usize> = (preferred..media.len()).collect();
    order.extend(0..preferred);
    let index = order
        .into_iter()
        .find(|&index| media_available(&media[index]))?;
    Some((media, index, name))
}

/// Restore the last playlist as a paused queue. No play count is recorded and
/// playback does not start by itself — the user presses play when ready.
/// If there is no usable session, load the first library item instead.
fn restore_playback_session(state: &AppState) {
    let session = {
        let conn = state.db.lock().unwrap();
        match Repository::get_setting(&conn, "playback_session") {
            Ok(Some(json)) => serde_json::from_str::<PlaybackSession>(&json).ok(),
            _ => None,
        }
    };

    let restored = session.and_then(|session| {
        if session.queue.is_empty() {
            return None;
        }
        let media: Vec<models::MediaItem> = {
            let conn = state.db.lock().unwrap();
            session
                .queue
                .iter()
                .filter_map(|id| Repository::get_media(&conn, *id).ok().flatten())
                .collect()
        };
        restorable_queue(media, session.current_index, session.name)
    });

    let Some((media, index, name)) = restored.or_else(|| {
        let media = {
            let conn = state.db.lock().unwrap();
            let filter = models::MediaFilter {
                kind: None,
                favorite: None,
                search: None,
                limit: Some(200),
                offset: None,
            };
            Repository::list_media(&conn, &filter).unwrap_or_default()
        };
        restorable_queue(media, 0, "Library".to_string())
    }) else {
        return;
    };

    let item = media[index].clone();
    state.player.lock().unwrap().set_queue(media, index, name);
    let _ = state.mpv.set_paused(true);
    if state.mpv.load(&item).is_ok() {
        let _ = state.mpv.set_paused(true);
    }
    state.emit_queue();
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    crate::widget::sync_with_main_window(app);
}

/// Import OS-provided files and play the first one, off the UI thread.
fn play_launched_files(state: AppState, paths: Vec<String>) {
    std::thread::spawn(move || {
        let playback_path = paths.first().cloned();
        if let Err(error) = commands::library::import_files_impl(&state, paths) {
            tracing::warn!("Could not import the launched file: {error}");
            return;
        }
        let Some(path) = playback_path else { return };
        let media = {
            let conn = state.db.lock().unwrap();
            Repository::get_media_by_path(&conn, &path)
        };
        match media {
            Ok(Some(item)) => {
                if let Err(error) = state.play_media(&item) {
                    tracing::warn!("Could not play the launched file: {error}");
                }
            }
            Ok(None) => tracing::warn!("Launched file was not added to the library."),
            Err(error) => tracing::warn!("Could not find the launched file: {error}"),
        }
    });
}

pub fn run() {
    tracing_subscriber::fmt().init();

    tauri::Builder::default()
        // Must be the first plugin: a second launch forwards its argv here and
        // exits, so "Open with" reuses the running window instead of opening a
        // duplicate application.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let paths = media_paths_from_args(argv);
            focus_main_window(app);
            if paths.is_empty() {
                return;
            }
            match app.try_state::<AppState>() {
                Some(state) => play_launched_files(state.inner().clone(), paths),
                None => {
                    tracing::warn!("A file was opened before the application finished starting.");
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                // Use the installer's product name so NSIS's default uninstall
                // section deletes this exact HKCU Run value.
                .app_name("Slide Up")
                .args(["--from-autostart"])
                .build(),
        )
        .setup(|app| {
            let data_dir = {
                #[cfg(debug_assertions)]
                {
                    app.path().app_data_dir()?.join("dev")
                }
                #[cfg(not(debug_assertions))]
                {
                    app.path().app_data_dir()?
                }
            };
            fs::create_dir_all(&data_dir)?;
            fs::create_dir_all(data_dir.join("thumbnails"))?;
            fs::create_dir_all(data_dir.join("storyboards"))?;
            fs::create_dir_all(data_dir.join("screenshots"))?;

            let database_path = data_dir.join("slideup.db");
            let connection = rusqlite::Connection::open(&database_path)?;
            schema::migrate(&connection)?;

            let settings = match Repository::get_setting(&connection, "app")? {
                Some(json) => serde_json::from_str::<AppSettings>(&json).unwrap_or_default(),
                None => AppSettings::default(),
            };

            let mpv = mpv::MpvClient::new(app.handle().clone(), &data_dir)?;
            // Offline caption model lifecycle. Managed so every command shares
            // one ONNX session and one download cancellation token.
            let caption_device = CaptionDevice::parse(&settings.caption_device);
            let models = model_manager::ModelManager::new(app.handle().clone(), &data_dir, caption_device)?;
            let upscalers = upscaler_manager::UpscalerManager::new(app.handle().clone(), &data_dir)?;
            let state = AppState::new(
                app.handle().clone(),
                connection,
                mpv,
                settings,
                data_dir,
            );
            app.manage((*state).clone());

            app.manage(models.clone());
            app.manage(upscalers.clone());

            // The SenseVoice/ONNX engine is intentionally NOT loaded here.
            // It is only initialized from the explicit "Load engine" action in
            // Settings → Live captions, keeping startup and the OS responsive.

            let startup = state.settings.lock().unwrap().clone();

            // Tray icon with a context menu. Tauri's tray manager already keeps
            // a reference alive; storing another clone in managed state would
            // survive `cleanup_before_exit()` and leave a ghost icon in the
            // Windows notification area when the process exits.
            let tray_handle = app.handle().clone();
            let toggle = MenuItem::with_id(&tray_handle, "tray_toggle", "Show / Hide", true, None::<&str>)?;
            let play_pause = MenuItem::with_id(&tray_handle, "tray_play_pause", "Play / Pause", true, None::<&str>)?;
            let next = MenuItem::with_id(&tray_handle, "tray_next", "Next", true, None::<&str>)?;
            let previous = MenuItem::with_id(&tray_handle, "tray_previous", "Previous", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(&tray_handle)?;
            let quit = MenuItem::with_id(&tray_handle, "tray_quit", "Quit", true, None::<&str>)?;

            let tray_menu = Menu::new(&tray_handle)?;
            tray_menu.append(&toggle)?;
            tray_menu.append(&play_pause)?;
            tray_menu.append(&next)?;
            tray_menu.append(&previous)?;
            tray_menu.append(&separator)?;
            tray_menu.append(&quit)?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("application icon").clone())
                .tooltip("Slide Up")
                .menu(&tray_menu)
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        crate::widget::sync_with_main_window(&tray_handle);
                    }
                })
                .on_menu_event(move |app, event| {
                    if event.id() == "tray_quit" {
                        let state = app.state::<AppState>();
                        let _ = state.mpv.stop();
                        // Return the shared video HWND before the video widget
                        // is torn down, for the same shutdown reason as above.
                        widget::destroy(app);
                        for (_, window) in app.webview_windows() {
                            let _ = window.destroy();
                        }
                        // Let the event loop end after the last window is
                        // destroyed. Calling exit() here uses
                        // std::process::exit and skips Chromium's window
                        // cleanup (Chrome_WidgetWin_0 / error 1412).
                        return;
                    }
                    if event.id() == "tray_toggle" {
                        let was_visible = app
                            .get_webview_window("main")
                            .map(|window| window.is_visible().unwrap_or(false))
                            .unwrap_or(false);
                        if let Some(window) = app.get_webview_window("main") {
                            if was_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        if was_visible {
                            crate::widget::show_if_enabled(app);
                        } else {
                            crate::widget::sync_with_main_window(app);
                        }
                        return;
                    }
                    let state = app.state::<AppState>();
                    let result = if event.id() == "tray_play_pause" {
                        state.mpv.toggle_pause()
                    } else if event.id() == "tray_next" {
                        state.next()
                    } else if event.id() == "tray_previous" {
                        state.previous()
                    } else {
                        Ok(())
                    };
                    if let Err(error) = result {
                        tracing::warn!("Tray playback command failed: {error}");
                    }
                })
                .build(app)?;

            // Window close handling and Windows taskbar buttons
            let close_handle = app.handle().clone();
            let close_state = state.clone();
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(windows)]
                taskbar::setup_taskbar_buttons(&window);

                window.on_window_event(move |event| {
                    // Covers restoring the main window from the taskbar or by
                    // clicking it: the widget must step aside when it returns.
                    if let tauri::WindowEvent::Focused(true) = event {
                        crate::widget::sync_with_main_window(&close_handle);
                    }
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let settings = close_state.settings.lock().unwrap().clone();
                        let is_playing = close_state.mpv.snapshot().status == models::PlayerStatus::Playing;

                        if settings.keep_widget_on_close && settings.player_widget {
                            // Opt-in mini-player mode: hide the main window but
                            // keep the widget and playback alive. A real quit is
                            // still available from the tray menu / taskbar Exit,
                            // both of which tear the widget down before exiting.
                            api.prevent_close();
                            if let Some(window) = close_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                            widget::show_if_enabled(&close_handle);
                        } else if settings.taskbar_controls && is_playing {
                            api.prevent_close();
                            if let Some(window) = close_handle.get_webview_window("main") {
                                let _ = window.minimize();
                            }
                            widget::show_if_enabled(&close_handle);
                        } else if settings.minimize_to_tray {
                            api.prevent_close();
                            if let Some(window) = close_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                            widget::show_if_enabled(&close_handle);
                        } else {
                            // This is a real quit. Stop playback and destroy the
                            // hidden widget so no WebView2 window is left open.
                            // Do not call exit(): it invokes std::process::exit
                            // while Chromium still owns windows, which causes
                            // "Failed to unregister class Chrome_WidgetWin_0.
                            // Error = 1412". With the widget gone, closing the
                            // main window leaves no windows and Tauri ends the
                            // event loop cleanly.
                            let _ = close_state.mpv.stop();
                            widget::destroy(&close_handle);
                        }
                    }
                });
            }

            // A sign-in launch shows only the desktop widget when one is
            // enabled; ordinary launches keep their configured visibility.
            let widget_only = started_from_autostart() && startup.player_widget;
            let main_hidden = startup.start_minimized || widget_only;
            if main_hidden {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else if let Some(window) = app.get_webview_window("main") {
                // Prepare splash geometry but leave the window hidden. The
                // inline HTML reveal shows it after its first paint; showing it
                // here made the splash flash at the old position before the
                // frontend centered it (a visible "two splashes" effect).
                let _ = window.set_size(tauri::LogicalSize::new(560.0, 340.0));
                let _ = window.center();
            }

            // Restore the optional desktop widget only after the main window
            // has its final visibility. Creating its WebView2 window before
            // that made it try to focus a window that was still hidden, which
            // could stall startup and leave the UI controls unresponsive.
            // With the full window open the widget stays out of the way.
            if main_hidden {
                widget::show_if_enabled(app.handle());
            } else {
                widget::sync_with_main_window(app.handle());
            }

            if startup.auto_scan_on_startup {
                if let Some(folder) = startup.default_media_folder.clone() {
                    let scan_state = state.clone();
                    std::thread::spawn(move || {
                        let _ = commands::library::scan_folder_impl(&scan_state, &folder);
                    });
                }
            }

            restore_playback_session(&state);

            // Files passed on the command line (file association / "Open with")
            // are imported and the first one starts playing.
            let launch_paths = media_paths_from_args(std::env::args());
            if !launch_paths.is_empty() {
                play_launched_files((*state).clone(), launch_paths);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_version,
            commands::app::open_external,
            commands::app::show_in_folder,
            commands::app::show_main_window,
            commands::app::show_widget_when_main_hidden,
            commands::app::set_launch_at_startup,
            commands::app::is_launch_at_startup,
            commands::app::fetch_url_text,
            commands::associations::get_file_associations,
            commands::associations::register_file_associations,
            commands::associations::unregister_file_associations,
            commands::associations::add_custom_extension,
            commands::associations::remove_custom_extension,
            commands::associations::open_default_apps_settings,
            model_manager::check_model_status,
            model_manager::download_caption_model,
            model_manager::cancel_model_download,
            model_manager::load_caption_engine,
            model_manager::unload_caption_engine,
            model_manager::set_caption_device,
            model_manager::delete_caption_model,
            upscaler_manager::list_upscaler_models,
            upscaler_manager::download_upscaler_model,
            upscaler_manager::cancel_upscaler_download,
            upscaler_manager::delete_upscaler_model,
            commands::library::pick_files,
            commands::library::pick_folder,
            commands::library::list_media,
            commands::library::get_recent,
            commands::library::clear_recent,
            commands::library::remove_from_recent,
            commands::library::get_media,
            commands::library::set_favorite,
            commands::library::delete_media,
            commands::library::clear_library,
            commands::library::scan_folder,
            commands::library::import_files,
            commands::library::get_thumbnail,
            commands::library::get_storyboard_frame,
            commands::library::generate_storyboard,
            commands::library::has_storyboard,
            commands::library::has_subtitle,
            commands::lyrics::get_lyrics,
            commands::lyrics::save_lyrics,
            commands::lyrics::search_lyrics,
            commands::playback::get_snapshot,
            commands::playback::list_audio_devices,
            commands::playback::play_media,
            commands::playback::play_queue,
            commands::playback::play_by_path,
            commands::playback::next,
            commands::playback::previous,
            commands::playback::toggle_pause,
            commands::playback::set_paused,
            commands::playback::stop,
            commands::playback::seek,
            commands::playback::seek_relative,
            commands::playback::set_volume,
            commands::playback::set_muted,
            commands::playback::set_speed,
            commands::playback::set_shuffle,
            commands::playback::set_repeat,
            commands::playback::set_fullscreen,
            commands::playback::toggle_pip,
            commands::playback::is_pip,
            commands::playback::get_tracks,
            commands::playback::set_video_track,
            commands::playback::set_audio_track,
            commands::playback::set_subtitle_track,
            commands::playback::add_subtitle,
            commands::playback::set_subtitle_delay,
            commands::playback::frame_step,
            commands::playback::set_aspect_ratio,
            commands::playback::set_ai_enhance,
            commands::playback::set_video_filter,
            commands::playback::auto_video_filters,
            commands::upscale::upscale_video_with_realesrgan,
            commands::playback::screenshot,
            commands::playback::sync_video_rect,
            commands::playback::set_widget_video,
            commands::playback::sync_widget_video_rect,
            commands::playback::set_surface_visible,
            commands::playback::set_video_layer,
            commands::playback::get_queue,
            commands::playback::queue_next,
            commands::playback::queue_add,
            commands::playback::queue_remove,
            commands::playback::queue_reorder,
            commands::playback::queue_clear,
            commands::playback::queue_play_at,
            commands::playlists::list_playlists,
            commands::playlists::create_playlist,
            commands::playlists::rename_playlist,
            commands::playlists::delete_playlist,
            commands::playlists::get_playlist_tracks,
            commands::playlists::add_to_playlist,
            commands::playlists::remove_from_playlist,
            commands::playlists::reorder_playlist,
            commands::playlists::play_playlist,
            commands::playlists::export_playlist,
            commands::playlists::import_playlist,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::reset_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SlideUp");
}
