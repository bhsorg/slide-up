import { computed, ref } from "vue";
import type { LyricLine, LyricsResult, LyricsSource } from "../types/lyrics";
import { findLyrics } from "../services/lyrics";
import { api } from "../services/tauri";

const LYRICS_VISIBLE_KEY = "slideup-lyrics-visible";
let preferencesLoaded = false;

export type LyricsStatus = "idle" | "loading" | "ready" | "notfound" | "error";

// One cache per media id, shared by every surface that shows lyrics.
const cache = new Map<number, LyricsResult | null>();

/** Whether the lyrics panel is currently shown (persists across view switches). */
const showLyrics = ref(false);

const lines = ref<LyricLine[]>([]);
const status = ref<LyricsStatus>("idle");
const synced = ref(false);
const trackId = ref<number | null>(null);
const activeIndex = ref(-1);
const title = ref("");
const artist = ref("");
const errorMessage = ref("");

function apply(result: LyricsResult | null, mediaId: number) {
  errorMessage.value = "";
  if (!result) {
    status.value = "notfound";
    lines.value = [];
    synced.value = false;
    activeIndex.value = -1;
    title.value = "";
    artist.value = "";
    return;
  }
  lines.value = result.lines;
  synced.value = result.synced;
  status.value = "ready";
  activeIndex.value = result.synced ? -1 : 0;
  title.value = result.trackName;
  artist.value = result.artistName;
  trackId.value = mediaId;
}

async function load(item: LyricsSource | null, force = false): Promise<void> {
  if (!item || !item.title) {
    clear();
    return;
  }
  if (!force && trackId.value === item.id && (status.value === "ready" || status.value === "loading")) {
    return;
  }

  trackId.value = item.id;
  activeIndex.value = -1;
  title.value = item.title;
  artist.value = item.artist ?? "";

  if (force) {
    cache.delete(item.id);
  } else {
    if (cache.has(item.id)) {
      const cached = cache.get(item.id);
      if (cached) {
        apply(cached, item.id);
        return;
      }
    }

    // Disk cache survives app restarts.
    const stored = await api.getLyrics(item.id).catch(() => null);
    if (trackId.value !== item.id) return;
    if (stored && stored.found && stored.result) {
      cache.set(item.id, stored.result);
      apply(stored.result, item.id);
      return;
    }
  }

  status.value = "loading";
  errorMessage.value = "";
  lines.value = [];
  synced.value = false;

  try {
    const result = await findLyrics(item);
    cache.set(item.id, result);
    // Persist real lyrics or empty result
    void api.saveLyrics(item.id, result).catch(() => {});
    if (trackId.value === item.id) apply(result, item.id);
  } catch (error) {
    if (trackId.value !== item.id) return;
    status.value = "error";
    errorMessage.value = error instanceof Error ? error.message : "Could not load lyrics.";
    lines.value = [];
  }
}

function clear(): void {
  trackId.value = null;
  lines.value = [];
  status.value = "idle";
  synced.value = false;
  activeIndex.value = -1;
  title.value = "";
  artist.value = "";
  errorMessage.value = "";
}

/** Update the highlighted line from the current playback position in ms. */
function updatePosition(positionMs: number): void {
  if (!synced.value || !lines.value.length) return;
  let next = -1;
  for (let i = 0; i < lines.value.length; i++) {
    if (lines.value[i].timeMs <= positionMs) next = i;
    else break;
  }
  if (next !== activeIndex.value) activeIndex.value = next;
}

function ensurePreferences(): void {
  if (preferencesLoaded) return;
  preferencesLoaded = true;
  try {
    showLyrics.value = localStorage.getItem(LYRICS_VISIBLE_KEY) === "1";
  } catch {
    showLyrics.value = false;
  }
}

function setShowLyrics(value: boolean): void {
  showLyrics.value = value;
  try {
    localStorage.setItem(LYRICS_VISIBLE_KEY, value ? "1" : "0");
  } catch {
    // Storage can be unavailable in hardened webviews; the toggle still works.
  }
}

const activeLine = computed<LyricLine | null>(() => {
  if (status.value !== "ready" || !lines.value.length) return null;
  const idx = activeIndex.value;
  if (idx >= 0 && idx < lines.value.length) return lines.value[idx];
  return lines.value[0] ?? null;
});

const prevLine = computed<LyricLine | null>(() => {
  if (status.value !== "ready" || !lines.value.length) return null;
  const idx = activeIndex.value;
  if (idx > 0 && idx < lines.value.length) return lines.value[idx - 1];
  return null;
});

const nextLine = computed<LyricLine | null>(() => {
  if (status.value !== "ready" || !lines.value.length) return null;
  const idx = activeIndex.value;
  if (idx >= 0 && idx + 1 < lines.value.length) return lines.value[idx + 1];
  return null;
});

export function useLyrics() {
  ensurePreferences();
  return {
    lines,
    status,
    synced,
    activeIndex,
    activeLine,
    prevLine,
    nextLine,
    title,
    artist,
    errorMessage,
    trackId,
    showLyrics,
    setShowLyrics,
    load,
    clear,
    updatePosition,
  };
}
