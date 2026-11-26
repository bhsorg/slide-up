import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, events, onEvent } from "../services/tauri";
import type { MediaFilter, MediaItem, ScanResult } from "../types/media";

export type SortKey = "added" | "title" | "artist" | "duration" | "plays" | "recent";

export const useLibraryStore = defineStore("library", () => {
  const items = ref<MediaItem[]>([]);
  const recent = ref<MediaItem[]>([]);
  const search = ref("");
  const matchedLyricsMediaIds = ref<Set<number>>(new Set());
  // Video covers are decrypted data URLs from the active workspace's cache.
  const videoCovers = ref<Record<number, string>>({});
  const coverPending = new Set<number>();
  const coverFailed = new Set<number>();
  const coverOrder: number[] = [];
  const COVER_CACHE_LIMIT = 200;
  const kindFilter = ref<"all" | "audio" | "video">("all");
  const favoritesOnly = ref(false);
  const sortBy = ref<SortKey>("added");
  const loading = ref(false);
  const scanning = ref(false);
  const scanStatus = ref("");
  const lastScan = ref<ScanResult | null>(null);
  const initialized = ref(false);
  const unlisteners: Array<() => void> = [];

  const isStream = (item: MediaItem) =>
    item.path.startsWith("http://") ||
    item.path.startsWith("https://") ||
    item.path.startsWith("rtmp://") ||
    item.path.startsWith("rtsp://");

  const audioItems = computed(() => items.value.filter((item) => item.kind === "audio" && !isStream(item)));
  const videoItems = computed(() => items.value.filter((item) => item.kind === "video" && !isStream(item)));
  const favorites = computed(() => items.value.filter((item) => item.favorite));

  const filtered = computed(() => {
    const needle = search.value.trim().toLowerCase();
    let result = items.value.filter((item) => {
      if (kindFilter.value !== "all" && item.kind !== kindFilter.value) return false;
      if (favoritesOnly.value && !item.favorite) return false;
      if (!needle) return true;
      if (matchedLyricsMediaIds.value.has(item.id)) return true;
      return [item.title, item.artist, item.album, item.codec]
        .filter(Boolean)
        .some((value) => value!.toLowerCase().includes(needle));
    });
    result = [...result];
    switch (sortBy.value) {
      case "title":
        result.sort((a, b) => a.title.localeCompare(b.title));
        break;
      case "artist":
        result.sort((a, b) => (a.artist ?? "~").localeCompare(b.artist ?? "~"));
        break;
      case "duration":
        result.sort((a, b) => b.durationMs - a.durationMs);
        break;
      case "plays":
        result.sort((a, b) => b.playCount - a.playCount);
        break;
      case "recent":
        result.sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0));
        break;
      default:
        result.sort((a, b) => b.addedAt - a.addedAt);
    }
    return result;
  });

  const visible = computed(() => filtered.value);

  function byId(id: number) {
    return items.value.find((item) => item.id === id) ?? null;
  }

  function thumbnailUrl(path: string | null): string | undefined {
    if (!path) return undefined;
    if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
      return path;
    }
    return convertFileSrc(path);
  }

  type CoverSource = Pick<MediaItem, "id" | "kind" | "thumbnailPath">;
  function coverUrl(item: CoverSource | null | undefined): string | undefined {
    if (!item) return undefined;
    if (item.kind === "video") {
      // Lazy generation: the first component that renders a video cover asks
      // the backend to decrypt or capture it once per media id.
      if (
        !videoCovers.value[item.id] &&
        !coverFailed.has(item.id) &&
        !coverPending.has(item.id)
      ) {
        void requestVideoCover(item);
      }
      return videoCovers.value[item.id];
    }
    return thumbnailUrl(item.thumbnailPath);
  }

  async function requestVideoCover(item: CoverSource) {
    if (
      item.kind !== "video" ||
      videoCovers.value[item.id] ||
      coverFailed.has(item.id) ||
      coverPending.has(item.id)
    ) {
      return;
    }
    coverPending.add(item.id);
    try {
      const url = await api.getVideoCover(item.id);
      if (url) {
        const existing = coverOrder.indexOf(item.id);
        if (existing >= 0) coverOrder.splice(existing, 1);
        coverOrder.push(item.id);
        videoCovers.value[item.id] = url;
        while (coverOrder.length > COVER_CACHE_LIMIT) {
          const oldest = coverOrder.shift();
          if (oldest !== undefined) delete videoCovers.value[oldest];
        }
      } else {
        // No cover could be generated; do not retry on every render.
        coverFailed.add(item.id);
      }
    } catch (error) {
      coverFailed.add(item.id);
      console.warn("getVideoCover failed:", error);
    } finally {
      coverPending.delete(item.id);
    }
  }

  function clearCovers() {
    videoCovers.value = {};
    coverPending.clear();
    coverFailed.clear();
    coverOrder.length = 0;
  }

  function resetFilters() {
    search.value = "";
    matchedLyricsMediaIds.value.clear();
    kindFilter.value = "all";
    favoritesOnly.value = false;
  }

  function resetLocal() {
    // Drop everything that came from the previous workspace before the new
    // workspace data is loaded.
    clearCovers();
    resetFilters();
    items.value = [];
    recent.value = [];
    scanning.value = false;
    scanStatus.value = "";
    lastScan.value = null;
  }

  // Debounced search for lyrics in database when search query changes
  let searchLyricsTimeout: ReturnType<typeof setTimeout> | null = null;
  watch(search, (query) => {
    if (searchLyricsTimeout) clearTimeout(searchLyricsTimeout);
    const trimmed = query.trim();
    if (trimmed.length < 2) {
      matchedLyricsMediaIds.value.clear();
      return;
    }
    searchLyricsTimeout = setTimeout(async () => {
      try {
        const ids = await api.searchLyrics(trimmed);
        matchedLyricsMediaIds.value = new Set(ids);
      } catch (err) {
        console.warn("searchLyrics failed:", err);
      }
    }, 200);
  });

  async function refresh() {
    loading.value = true;
    try {
      const filter: MediaFilter = { kind: "all" };
      const [all, recentItems] = await Promise.all([api.listMedia(filter), api.getRecent(16)]);
      items.value = all;
      recent.value = recentItems;
    } finally {
      loading.value = false;
    }
  }

  async function initialize() {
    if (initialized.value) return;
    initialized.value = true;
    await refresh();
    unlisteners.push(
      await onEvent<void>(events.libraryChanged, () => {
        clearCovers();
        void refresh();
      }),
      await onEvent<void>(events.workspaceChanged, () => {
        // Never leave the previous workspace's media, recents or covers visible.
        resetLocal();
        void refresh();
      }),
      await onEvent<string>(events.scanProgress, (message) => {
        scanStatus.value = message;
      }),
    );
  }

  function dispose() {
    unlisteners.forEach((unlisten) => unlisten());
    unlisteners.length = 0;
    initialized.value = false;
  }

  async function importFiles() {
    const paths = await api.pickFiles();
    if (!paths.length) return null;
    scanning.value = true;
    scanStatus.value = `Importing ${paths.length} file(s)…`;
    try {
      const result = await api.importFiles(paths);
      await refresh();
      return result;
    } finally {
      scanning.value = false;
      scanStatus.value = "";
    }
  }

  async function scanFolder(folder?: string) {
    const target = folder ?? (await api.pickFolder());
    if (!target) return null;
    scanning.value = true;
    scanStatus.value = `Scanning ${target}…`;
    try {
      const result = await api.scanFolder(target);
      lastScan.value = result;
      await refresh();
      return result;
    } finally {
      scanning.value = false;
      scanStatus.value = "";
    }
  }

  async function importPaths(paths: string[]) {
    if (!paths.length) return null;
    scanning.value = true;
    scanStatus.value = `Importing ${paths.length} file(s)…`;
    try {
      const result = await api.importFiles(paths);
      await refresh();
      return result;
    } finally {
      scanning.value = false;
      scanStatus.value = "";
    }
  }

  async function toggleFavorite(item: MediaItem) {
    const next = !item.favorite;
    item.favorite = next;
    try {
      await api.setFavorite(item.id, next);
    } catch (error) {
      item.favorite = !next;
      throw error;
    }
  }

  async function remove(item: MediaItem, deleteFile = false) {
    await api.deleteMedia(item.id, deleteFile);
    await refresh();
  }

  async function removeMany(ids: number[], deleteFiles = false) {
    for (const id of ids) {
      await api.deleteMedia(id, deleteFiles);
    }
    await refresh();
  }

  async function clearRecent() {
    try {
      await api.clearRecent();
    } catch (e) {
      console.warn("clearRecent error:", e);
    }
    recent.value = [];
    await refresh();
  }

  async function removeFromRecent(ids: number[]) {
    try {
      await api.removeFromRecent(ids);
    } catch (e) {
      console.warn("removeFromRecent error:", e);
    }
    const idSet = new Set(ids);
    recent.value = recent.value.filter((item) => !idSet.has(item.id));
    await refresh();
  }

  async function removeFavorites(ids: number[]) {
    for (const id of ids) {
      await api.setFavorite(id, false);
    }
    await refresh();
  }

  async function clear(kind: "audio" | "video") {
    const removed = await api.clearLibrary(kind);
    await refresh();
    return removed;
  }

  return {
    items,
    recent,
    search,
    matchedLyricsMediaIds,
    kindFilter,
    favoritesOnly,
    sortBy,
    loading,
    scanning,
    scanStatus,
    lastScan,
    audioItems,
    videoItems,
    favorites,
    filtered,
    visible,
    byId,
    thumbnailUrl,
    videoCovers,
    coverUrl,
    requestVideoCover,
    clearCovers,
    resetLocal,
    resetFilters,
    refresh,
    initialize,
    dispose,
    importFiles,
    scanFolder,
    importPaths,
    toggleFavorite,
    remove,
    removeMany,
    clearRecent,
    removeFromRecent,
    removeFavorites,
    clear,
  };
});
