import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api, events, onEvent } from "../services/tauri";
import type { MediaItem } from "../types/media";
import type { Playlist, PlaylistTrack } from "../types/playlist";

export const usePlaylistStore = defineStore("playlist", () => {
  const playlists = ref<Playlist[]>([]);
  const activeId = ref<number | null>(null);
  const tracks = ref<PlaylistTrack[]>([]);
  const loading = ref(false);
  const initialized = ref(false);
  const unlisteners: Array<() => void> = [];

  const active = computed(() => playlists.value.find((p) => p.id === activeId.value) ?? null);

  async function refresh() {
    loading.value = true;
    try {
      playlists.value = await api.listPlaylists();
      if (activeId.value && !playlists.value.some((p) => p.id === activeId.value)) {
        activeId.value = playlists.value[0]?.id ?? null;
      }
      if (activeId.value == null && playlists.value.length) {
        activeId.value = playlists.value[0].id;
      }
      if (activeId.value != null) await loadTracks(activeId.value);
      else tracks.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function initialize() {
    if (initialized.value) return;
    initialized.value = true;
    await refresh();
    unlisteners.push(await onEvent<void>(events.playlistsChanged, () => refresh()));
  }

  function dispose() {
    unlisteners.forEach((unlisten) => unlisten());
    unlisteners.length = 0;
    initialized.value = false;
  }

  async function loadTracks(id: number) {
    tracks.value = await api.getPlaylistTracks(id);
  }

  async function select(id: number) {
    activeId.value = id;
    await loadTracks(id);
  }

  async function create(name: string) {
    const id = await api.createPlaylist(name.trim() || "New Playlist");
    await refresh();
    await select(id);
    return id;
  }

  async function rename(id: number, name: string) {
    await api.renamePlaylist(id, name.trim());
    await refresh();
  }

  async function remove(id: number) {
    await api.deletePlaylist(id);
    if (activeId.value === id) activeId.value = null;
    await refresh();
  }

  async function addMedia(id: number, media: MediaItem) {
    await api.addToPlaylist(id, media.id);
    if (id === activeId.value) await loadTracks(id);
    await refresh();
  }

  async function removeMedia(id: number, media: MediaItem) {
    await api.removeFromPlaylist(id, media.id);
    if (id === activeId.value) await loadTracks(id);
    await refresh();
  }

  async function reorder(id: number, ids: number[]) {
    await api.reorderPlaylist(id, ids);
    await loadTracks(id);
    await refresh();
  }

  async function play(id: number, index = 0) {
    await api.playPlaylist(id, index);
  }

  async function exportPlaylist(id: number, name: string) {
    return api.exportPlaylist(id, name);
  }

  async function importPlaylist(name?: string) {
    const id = await api.importPlaylist(name);
    await refresh();
    if (id != null) await select(id);
    return id;
  }

  return {
    playlists,
    activeId,
    tracks,
    loading,
    active,
    refresh,
    initialize,
    dispose,
    loadTracks,
    select,
    create,
    rename,
    remove,
    addMedia,
    removeMedia,
    reorder,
    play,
    exportPlaylist,
    importPlaylist,
  };
});
