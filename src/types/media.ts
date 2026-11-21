export type MediaKind = "audio" | "video";

export interface MediaItem {
  id: number;
  path: string;
  kind: MediaKind;
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number;
  codec: string | null;
  width: number | null;
  height: number | null;
  sizeBytes: number;
  thumbnailPath: string | null;
  favorite: boolean;
  playCount: number;
  lastPlayedAt: number | null;
  addedAt: number;
}

export interface MediaFilter {
  kind?: MediaKind | "all";
  favorite?: boolean;
  search?: string;
  limit?: number;
  offset?: number;
}

export interface ScanResult {
  found: number;
  added: number;
  updated: number;
  removed: number;
}

export const formatDuration = (ms: number): string => {
  if (!Number.isFinite(ms) || ms < 0) return "--:--";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
};

export const formatBytes = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(1)} ${units[i]}`;
};

export const isAudioFile = (path: string): boolean =>
  /\.(mp3|wav|flac|aac|ogg|oga|m4a|opus|wma|aiff?|alac|ape)$/i.test(path);

export const isVideoFile = (path: string): boolean =>
  /\.(mp4|mkv|avi|mov|webm|m4v|wmv|flv|ts|m2ts|mpg|mpeg|3gp|ogv)$/i.test(path);

export const isMediaFile = (path: string): boolean => isAudioFile(path) || isVideoFile(path);
