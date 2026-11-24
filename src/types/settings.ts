export type Theme = "dark" | "light";
export type ViewId =
  | "home"
  | "tv"
  | "music"
  | "videos"
  | "playlists"
  | "favorites"
  | "recent"
  | "settings"
  | "about";

export interface Shortcuts {
  playPause: string;
  seekBack: string;
  seekForward: string;
  volumeUp: string;
  volumeDown: string;
  fullscreen: string;
  mute: string;
  next: string;
  previous: string;
  exitFullscreen: string;
  openFile: string;
  openFolder: string;
}

export interface AudioDevice {
  name: string;
  description: string;
}

/** A user-added extension. Built-in formats are never stored here. */
export interface CustomExtension {
  ext: string;
  kind: "audio" | "video";
}

export interface FileExtension {
  ext: string;
  kind: "audio" | "video";
  registered: boolean;
  custom: boolean;
}

export interface FileAssociations {
  /** True when every built-in format is associated with Slide Up. */
  registered: boolean;
  audio: FileExtension[];
  video: FileExtension[];
  custom: FileExtension[];
}

export interface AppSettings {
  theme: Theme;
  defaultMediaFolder: string | null;
  volume: number;
  muted: boolean;
  playbackSpeed: number;
  shuffle: boolean;
  repeat: "off" | "all" | "one";
  hardwareAcceleration: boolean;
  audioOutput: string;
  audioDevice: string;
  subtitleFont: string;
  subtitleSize: number;
  subtitleColor: string;
  subtitleBorderSize: number;
  subtitleMargin: number;
  subtitleDelayMs: number;
  scanRecursive: boolean;
  scanHidden: boolean;
  scanAudio: boolean;
  scanVideo: boolean;
  generateThumbnails: boolean;
  removeMissingOnScan: boolean;
  autoScanOnStartup: boolean;
  startMinimized: boolean;
  minimizeToTray: boolean;
  taskbarControls: boolean;
  /** Show the compact always-on-top player widget on the desktop. */
  playerWidget: boolean;
  /** Opacity while the pointer is outside the desktop player widget (0-100). */
  playerWidgetMinOpacity: number;
  /** Opacity while the pointer is over the desktop player widget (0-100). */
  playerWidgetMaxOpacity: number;
  customExtensions: CustomExtension[];
  /** Whether live captions are enabled (requires the model to be downloaded). */
  liveCaptionsEnabled: boolean;
  /** SenseVoice execution provider: auto (GPU with CPU fallback), cpu or gpu. */
  captionDevice: "auto" | "cpu" | "gpu";
  videoBrightness: number;
  videoContrast: number;
  videoSaturation: number;
  videoGamma: number;
  transparentFilterDialog: boolean;
  shortcuts: Shortcuts;
}

export const defaultShortcuts = (): Shortcuts => ({
  playPause: "Space",
  seekBack: "ArrowLeft",
  seekForward: "ArrowRight",
  volumeUp: "ArrowUp",
  volumeDown: "ArrowDown",
  fullscreen: "f",
  mute: "m",
  next: "n",
  previous: "p",
  exitFullscreen: "Escape",
  openFile: "Ctrl+O",
  openFolder: "Ctrl+Shift+O",
});

export const defaultSettings = (): AppSettings => ({
  theme: "dark",
  defaultMediaFolder: null,
  volume: 0.8,
  muted: false,
  playbackSpeed: 1,
  shuffle: false,
  repeat: "off",
  hardwareAcceleration: true,
  audioOutput: "auto",
  audioDevice: "auto",
  subtitleFont: "Sans",
  subtitleSize: 22,
  subtitleColor: "#FFFFFF",
  subtitleBorderSize: 3,
  subtitleMargin: 24,
  subtitleDelayMs: 0,
  scanRecursive: true,
  scanHidden: false,
  scanAudio: true,
  scanVideo: true,
  generateThumbnails: true,
  removeMissingOnScan: false,
  autoScanOnStartup: false,
  startMinimized: false,
  minimizeToTray: false,
  taskbarControls: false,
  playerWidget: false,
  playerWidgetMinOpacity: 10,
  playerWidgetMaxOpacity: 100,
  customExtensions: [],
  liveCaptionsEnabled: false,
  captionDevice: "auto",
  videoBrightness: 0,
  videoContrast: 0,
  videoSaturation: 0,
  videoGamma: 0,
  transparentFilterDialog: false,
  shortcuts: defaultShortcuts(),
});
