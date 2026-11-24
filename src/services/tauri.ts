import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppSettings, AudioDevice, FileAssociations } from "../types/settings";
import type { MediaFilter, MediaItem, ScanResult } from "../types/media";
import type { PlayerSnapshot, MediaTrack, PlayMode, PlayerState } from "../types/player";
import type { Playlist, PlaylistTrack } from "../types/playlist";
import type { ModelStatus, UpscalerModelStatus } from "../types/model";
import type { CachedLyrics, LyricsResult } from "../types/lyrics";

export interface AutoVideoFilters {
  brightness: number;
  contrast: number;
  saturation: number;
  gamma: number;
}

export interface AppErrorPayload {
  code: string;
  message: string;
  title: string;
}

export class MediaPlayerError extends Error {
  code: string;
  title: string;
  constructor(payload: AppErrorPayload) {
    super(payload.message);
    this.code = payload.code;
    this.title = payload.title;
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (error && typeof error === "object" && "message" in error) {
      const payload = error as AppErrorPayload;
      if (payload.code && payload.title) throw new MediaPlayerError(payload);
      throw new Error(String(payload.message));
    }
    throw new Error(String(error));
  }
}

export const api = {
  appVersion: () => call<string>("app_version"),
  getAppVersion: () => call<string>("app_version"),
  openExternal: (url: string) => call<void>("open_external", { url }),
  showInFolder: (path: string) => call<void>("show_in_folder", { path }),
  fetchUrlText: (url: string) => call<string>("fetch_url_text", { url }),

  pickFiles: () => call<string[]>("pick_files"),
  pickFolder: () => call<string | null>("pick_folder"),
  listMedia: (filter: MediaFilter) => call<MediaItem[]>("list_media", { filter }),
  getRecent: (limit = 12) => call<MediaItem[]>("get_recent", { limit }),
  clearRecent: () => call<void>("clear_recent"),
  removeFromRecent: (ids: number[]) => call<void>("remove_from_recent", { ids }),
  getMedia: (id: number) => call<MediaItem | null>("get_media", { id }),
  setFavorite: (id: number, favorite: boolean) =>
    call<void>("set_favorite", { id, favorite }),
  deleteMedia: (id: number, deleteFile = false) =>
    call<void>("delete_media", { id, deleteFile }),
  clearLibrary: (kind: "audio" | "video") => call<number>("clear_library", { kind }),
  scanFolder: (folder: string) => call<ScanResult>("scan_folder", { folder }),
  importFiles: (paths: string[]) => call<ScanResult>("import_files", { paths }),

  getSnapshot: () => call<PlayerState>("get_snapshot"),
  playMedia: (media: MediaItem) => call<void>("play_media", { media }),
  playQueue: (media: MediaItem[], index: number, name: string) =>
    call<void>("play_queue", { media, index, name }),
  playByPath: (path: string) => call<void>("play_by_path", { path }),
  next: () => call<void>("next"),
  previous: () => call<void>("previous"),
  togglePause: () => call<void>("toggle_pause"),
  setPaused: (paused: boolean) => call<void>("set_paused", { paused }),
  stop: () => call<void>("stop"),
  seek: (positionMs: number) => call<void>("seek", { positionMs }),
  seekRelative: (deltaMs: number) => call<void>("seek_relative", { deltaMs }),
  setVolume: (volume: number) => call<void>("set_volume", { volume }),
  setMuted: (muted: boolean) => call<void>("set_muted", { muted }),
  setSpeed: (speed: number) => call<void>("set_speed", { speed }),
  setShuffle: (shuffle: boolean) => call<void>("set_shuffle", { shuffle }),
  setRepeat: (repeat: PlayMode) => call<void>("set_repeat", { repeat }),
  setFullscreen: (fullscreen: boolean) => call<void>("set_fullscreen", { fullscreen }),
  togglePip: () => call<boolean>("toggle_pip"),
  isPip: () => call<boolean>("is_pip"),
  getTracks: () => call<TrackPayload>("get_tracks"),
  listAudioDevices: () => call<AudioDevice[]>("list_audio_devices"),
  setVideoTrack: (id: number) => call<void>("set_video_track", { id }),
  setAudioTrack: (id: number) => call<void>("set_audio_track", { id }),
  setSubtitleTrack: (id: number) => call<void>("set_subtitle_track", { id }),
  addSubtitle: (path: string) => call<void>("add_subtitle", { path }),
  setSubtitleDelay: (delayMs: number) => call<void>("set_subtitle_delay", { delayMs }),
  frameStep: () => call<void>("frame_step"),
  setAspectRatio: (ratio: string) => call<void>("set_aspect_ratio", { ratio }),
  setAiEnhance: (enabled: boolean, modelId?: string) =>
    call<boolean>("set_ai_enhance", { enabled, modelId: modelId ?? null }),
  screenshot: () => call<string>("screenshot"),
  syncVideoRect: (x: number, y: number, width: number, height: number) =>
    call<void>("sync_video_rect", { x, y, width, height }),
  setWidgetVideo: (active: boolean) => call<void>("set_widget_video", { active }),
  syncWidgetVideoRect: (x: number, y: number, width: number, height: number, radius: number) =>
    call<void>("sync_widget_video_rect", { x, y, width, height, radius }),
  setSurfaceVisible: (visible: boolean) => call<void>("set_surface_visible", { visible }),
  setVideoLayer: (top: boolean) => call<void>("set_video_layer", { top }),
  setVideoFilter: (property: string, value: number) =>
    call<void>("set_video_filter", { property, value }),
  autoVideoFilters: () => call<AutoVideoFilters>("auto_video_filters"),
  getLyrics: (mediaId: number) => call<CachedLyrics | null>("get_lyrics", { mediaId }),
  saveLyrics: (mediaId: number, result: LyricsResult | null) =>
    call<void>("save_lyrics", { mediaId, result }),
  searchLyrics: (query: string) => call<number[]>("search_lyrics", { query }),
  getStoryboardFrame: (mediaId: number, index: number) =>
    call<string | null>("get_storyboard_frame", { mediaId, index }),
  generateStoryboard: (mediaId: number) =>
    call<void>("generate_storyboard", { mediaId }),
  hasStoryboard: (mediaId: number) => call<boolean>("has_storyboard", { mediaId }),
  hasSubtitle: (mediaId: number) => call<boolean>("has_subtitle", { mediaId }),

  getQueue: () => call<QueuePayload>("get_queue"),
  queueNext: (media: MediaItem) => call<void>("queue_next", { media }),
  queueAdd: (media: MediaItem) => call<void>("queue_add", { media }),
  queueRemove: (index: number) => call<void>("queue_remove", { index }),
  queueReorder: (from: number, to: number) => call<void>("queue_reorder", { from, to }),
  queueClear: () => call<void>("queue_clear"),
  queuePlayAt: (index: number) => call<void>("queue_play_at", { index }),

  listPlaylists: () => call<Playlist[]>("list_playlists"),
  createPlaylist: (name: string) => call<number>("create_playlist", { name }),
  renamePlaylist: (id: number, name: string) =>
    call<void>("rename_playlist", { id, name }),
  deletePlaylist: (id: number) => call<void>("delete_playlist", { id }),
  getPlaylistTracks: (playlistId: number) =>
    call<PlaylistTrack[]>("get_playlist_tracks", { playlistId }),
  addToPlaylist: (playlistId: number, mediaId: number) =>
    call<void>("add_to_playlist", { playlistId, mediaId }),
  removeFromPlaylist: (playlistId: number, mediaId: number) =>
    call<void>("remove_from_playlist", { playlistId, mediaId }),
  reorderPlaylist: (playlistId: number, orderedIds: number[]) =>
    call<void>("reorder_playlist", { playlistId, orderedIds }),
  playPlaylist: (playlistId: number, index: number) =>
    call<void>("play_playlist", { playlistId, index }),
  exportPlaylist: (playlistId: number, name: string) =>
    call<string | null>("export_playlist", { playlistId, name }),
  importPlaylist: (playlistName?: string) =>
    call<number | null>("import_playlist", { playlistName: playlistName ?? null }),

  getSettings: () => call<AppSettings>("get_settings"),
  updateSettings: (settings: AppSettings) => call<void>("update_settings", { settings }),
  resetSettings: () => call<AppSettings>("reset_settings"),

  getFileAssociations: () => call<FileAssociations>("get_file_associations"),
  registerFileAssociations: () => call<FileAssociations>("register_file_associations"),
  unregisterFileAssociations: () => call<FileAssociations>("unregister_file_associations"),
  addCustomExtension: (ext: string, kind: "audio" | "video") =>
    call<FileAssociations>("add_custom_extension", { ext, kind }),
  removeCustomExtension: (ext: string) =>
    call<FileAssociations>("remove_custom_extension", { ext }),
  openDefaultAppsSettings: () => call<void>("open_default_apps_settings"),

  checkModelStatus: () => call<ModelStatus>("check_model_status"),
  downloadCaptionModel: () => call<void>("download_caption_model"),
  cancelModelDownload: () => call<void>("cancel_model_download"),
  loadCaptionEngine: () => call<string>("load_caption_engine"),
  unloadCaptionEngine: () => call<void>("unload_caption_engine"),
  setCaptionDevice: (device: "auto" | "cpu" | "gpu") =>
    call<string>("set_caption_device", { device }),
  deleteCaptionModel: () => call<void>("delete_caption_model"),

  listUpscalerModels: () => call<UpscalerModelStatus[]>("list_upscaler_models"),
  downloadUpscalerModel: (id: string) => call<void>("download_upscaler_model", { id }),
  cancelUpscalerDownload: () => call<void>("cancel_upscaler_download"),
  deleteUpscalerModel: (id: string) => call<void>("delete_upscaler_model", { id }),
  upscaleVideoWithRealEsrgan: (path: string) =>
    call<string>("upscale_video_with_realesrgan", { path }),
};

export interface TrackPayload {
  videoTracks?: MediaTrack[];
  audioTracks: MediaTrack[];
  subtitleTracks: MediaTrack[];
  activeVideoTrack?: number;
  activeAudioTrack: number;
  activeSubtitleTrack: number;
}

export interface QueuePayload {
  queue: MediaItem[];
  currentIndex: number | null;
  name: string;
}

export const events = {
  playerState: "player://state",
  playerProgress: "player://progress",
  playerTracks: "player://tracks",
  queueChanged: "queue://changed",
  playerFileLoaded: "player://file-loaded",
  playerEnded: "player://ended",
  playerError: "player://error",
  libraryChanged: "library://changed",
  scanProgress: "library://scan-progress",
  storyboardProgress: "storyboard://progress",
  playlistsChanged: "playlists://changed",
  videoWidgetVisibilityChanged: "widget://video-visibility",
  playlistWidgetVisibilityChanged: "widget://playlist-visibility",
  settingsChanged: "settings://changed",
  toast: "app://toast",
  modelDownloadProgress: "model-download-progress",
  upscalerDownloadProgress: "upscaler-download-progress",
  upscaleProgress: "upscale-progress",
};

export function onEvent<T>(event: string, handler: (payload: T) => void) {
  return listen<T>(event, (eventObject) => handler(eventObject.payload));
}

export function hasProgressShape(payload: unknown): payload is PlayerSnapshot {
  return !!payload && typeof payload === "object" && "positionMs" in payload;
}
