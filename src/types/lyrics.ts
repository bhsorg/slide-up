/** Minimal track metadata needed to look up lyrics (MediaItem / TrackInfo). */
export interface LyricsSource {
  id: number;
  kind?: "audio" | "video";
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number;
}

/** A single lyric line. `timeMs` is -1 for unsynced (plain) lyrics. */
export interface LyricLine {
  timeMs: number;
  text: string;
}

/** Subset of the LRCLIB v1 lyrics record we actually use. */
export interface LrclibRecord {
  id: number;
  trackName: string;
  artistName: string;
  albumName: string | null;
  duration: number;
  instrumental: boolean;
  plainLyrics: string | null;
  syncedLyrics: string | null;
}

export interface LyricsResult {
  lines: LyricLine[];
  synced: boolean;
  trackName: string;
  artistName: string;
}

/** Disk cache envelope (Rust `CachedLyrics`). */
export interface CachedLyrics {
  found: boolean;
  result: LyricsResult | null;
}
