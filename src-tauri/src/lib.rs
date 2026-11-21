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

use db::schema;
use db::Repository;
use models::AppSettings;
use state::AppState;
use std::fs;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
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
            let models = model_manager::ModelManager::new(app.handle().clone(), &data_dir)?;
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
            if models.model_files_ready() {
                let startup_models = models.clone();
                std::thread::spawn(move || {
                    if let Err(error) = startup_models.initialize_engine() {
                        tracing::warn!("Could not auto-load the caption engine: {error}");
                    }
                });
            }

            let startup = state.settings.lock().unwrap().clone();

            // Tray icon with a context menu. The TrayIcon handle must stay
            // alive for the icon to exist, so it is stored in app state.
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

            let tray = TrayIconBuilder::new()
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
                    }
                })
                .on_menu_event(move |app, event| {
                    if event.id() == "tray_quit" {
                        app.exit(0);
                        return;
                    }
                    if event.id() == "tray_toggle" {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
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
            app.manage(tray);

            // Close button hides to the tray; use the tray Quit item to exit.
            let close_handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(window) = close_handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                });
            }

            if startup.start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            if startup.auto_scan_on_startup {
                if let Some(folder) = startup.default_media_folder.clone() {
                    let scan_state = state.clone();
                    std::thread::spawn(move || {
                        let _ = commands::library::scan_folder_impl(&scan_state, &folder);
                    });
                }
            }

            // Files passed on the command line (file association / "Open with")
            // are imported and the first one starts playing.
            let launch_paths: Vec<String> = std::env::args()
                .skip(1)
                .filter(|arg| !arg.starts_with('-'))
                .collect();
            if !launch_paths.is_empty() {
                let launch_state = state.clone();
                std::thread::spawn(move || {
                    let playback_path = launch_paths.first().cloned();
                    if let Err(error) =
                        commands::library::import_files_impl(&launch_state, launch_paths)
                    {
                        tracing::warn!("Could not import the launched file: {error}");
                        return;
                    }
                    let Some(path) = playback_path else { return };
                    let media = {
                        let conn = launch_state.db.lock().unwrap();
                        Repository::get_media_by_path(&conn, &path)
                    };
                    match media {
                        Ok(Some(item)) => {
                            if let Err(error) = launch_state.play_media(&item) {
                                tracing::warn!("Could not play the launched file: {error}");
                            }
                        }
                        Ok(None) => tracing::warn!("Launched file was not added to the library."),
                        Err(error) => tracing::warn!("Could not find the launched file: {error}"),
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_version,
            commands::app::open_external,
            commands::app::show_in_folder,
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
            model_manager::delete_caption_model,
            upscaler_manager::list_upscaler_models,
            upscaler_manager::download_upscaler_model,
            upscaler_manager::cancel_upscaler_download,
            upscaler_manager::delete_upscaler_model,
            commands::library::pick_files,
            commands::library::pick_folder,
            commands::library::list_media,
            commands::library::get_recent,
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
            commands::playback::set_audio_track,
            commands::playback::set_subtitle_track,
            commands::playback::add_subtitle,
            commands::playback::set_subtitle_delay,
            commands::playback::frame_step,
            commands::playback::set_aspect_ratio,
            commands::playback::set_ai_enhance,
            commands::playback::screenshot,
            commands::playback::sync_video_rect,
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
