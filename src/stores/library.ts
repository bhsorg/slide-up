import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, events, onEvent } from "../services/tauri";
import type { MediaFilter, MediaItem, ScanResult } from "../types/media";

export type SortKey = "added" | "title" | "artist" | "duration" | "plays" | "recent";

export const useLibraryStore = defineStore("library", () => {
  const items = ref<MediaItem[]>([]);
  const recent = ref<MediaItem[]>([]);
  const search = ref("");
  const kindFilter = ref<"all" | "audio" | "video">("all");
  const favoritesOnly = ref(false);
  const sortBy = ref<SortKey>("added");
  const loading = ref(false);
  const scanning = ref(false);
  const scanStatus = ref("");
  const lastScan = ref<ScanResult | null>(null);
  const initialized = ref(false);
  const unlisteners: Array<() => void> = [];

  const audioItems = computed(() => items.value.filter((item) => item.kind === "audio"));
  const videoItems = computed(() => items.value.filter((item) => item.kind === "video"));
  const favorites = computed(() => items.value.filter((item) => item.favorite));

  const filtered = computed(() => {
    const needle = search.value.trim().toLowerCase();
    let result = items.value.filter((item) => {
      if (kindFilter.value !== "all" && item.kind !== kindFilter.value) return false;
      if (favoritesOnly.value && !item.favorite) return false;
      if (!needle) return true;
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
    return path ? convertFileSrc(path) : undefined;
  }

  function resetFilters() {
    search.value = "";
    kindFilter.value = "all";
    favoritesOnly.value = false;
  }

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
      await onEvent<void>(events.libraryChanged, () => refresh()),
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

  async function clear(kind: "audio" | "video") {
    const removed = await api.clearLibrary(kind);
    await refresh();
    return removed;
  }

  return {
    items,
    recent,
    search,
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
    resetFilters,
    refresh,
    initialize,
    dispose,
    importFiles,
    scanFolder,
    importPaths,
    toggleFavorite,
    remove,
    clear,
  };
});
