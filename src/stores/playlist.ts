import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api, events, onEvent } from "../services/tauri";
import { usePlayerStore } from "./player";
import { parseM3u, normalizeMediaUrl } from "../utils/m3u";
import { parseTorrent } from "../utils/torrent";
import type { MediaItem } from "../types/media";
import type { Playlist, PlaylistTrack } from "../types/playlist";

export interface CustomM3uPlaylist {
  id: number;
  name: string;
  sourceUrl?: string;
  coverPath?: string | null;
  createdAt: number;
  updatedAt: number;
  tracks: MediaItem[];
}

const STORAGE_KEY = "slideup_custom_m3u_playlists";

function loadCustomPlaylists(): CustomM3uPlaylist[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveCustomPlaylists(list: CustomM3uPlaylist[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
  } catch {
    // quota fallback
  }
}

export const usePlaylistStore = defineStore("playlist", () => {
  const playlists = ref<Playlist[]>([]);
  const activeId = ref<number | null>(null);
  const tracks = ref<PlaylistTrack[]>([]);
  const loading = ref(false);
  const initialized = ref(false);
  const privateVisible = ref(false);
  const unlisteners: Array<() => void> = [];

  const active = computed(() => playlists.value.find((p) => p.id === activeId.value) ?? null);

  async function refresh() {
    loading.value = true;
    try {
      const dbPlaylists = await api.listPlaylists().catch(() => [] as Playlist[]);
      const custom = loadCustomPlaylists();
      const customAsPlaylists: Playlist[] = custom.map((p) => ({
        id: p.id,
        name: p.name,
        createdAt: p.createdAt,
        updatedAt: p.updatedAt,
        trackCount: p.tracks.length,
        coverPath: p.coverPath ?? p.tracks.find((t) => t.thumbnailPath)?.thumbnailPath ?? null,
        locked: false,
      }));

      playlists.value = [...customAsPlaylists, ...dbPlaylists];

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
    if (id < 0) {
      const custom = loadCustomPlaylists().find((p) => p.id === id);
      if (custom) {
        let hasUpdatedOldTorrentPaths = false;
        tracks.value = custom.tracks.map((media, index) => {
          if (media.path.startsWith("http://") || media.path.startsWith("https://")) {
            media.path = normalizeMediaUrl(media.path);
          } else if (media.path.startsWith("torrent://")) {
            // Auto-upgrade legacy torrent:// paths from Archive.org
            const rawPath = decodeURIComponent(media.path.replace(/^torrent:\/\/[^/]+\//, ""));
            const archiveName = (custom.name || media.album || "").trim().replace(/\s+/g, "-");
            if (archiveName) {
              media.path = normalizeMediaUrl(`https://archive.org/download/${encodeURIComponent(archiveName)}/${encodeURIComponent(rawPath)}`);
              hasUpdatedOldTorrentPaths = true;
            }
          }
          return {
            media,
            position: index,
          };
        });
        if (hasUpdatedOldTorrentPaths) {
          saveCustomPlaylists(loadCustomPlaylists().map((p) => (p.id === id ? { ...p, tracks: custom.tracks } : p)));
        }
      } else {
        tracks.value = [];
      }
    } else {
      tracks.value = await api.getPlaylistTracks(id);
    }
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
    if (id < 0) {
      const custom = loadCustomPlaylists();
      const target = custom.find((p) => p.id === id);
      if (target) {
        target.name = name.trim();
        target.updatedAt = Date.now();
        saveCustomPlaylists(custom);
        await refresh();
      }
    } else {
      await api.renamePlaylist(id, name.trim());
      await refresh();
    }
  }

  async function remove(id: number) {
    if (id < 0) {
      const custom = loadCustomPlaylists().filter((p) => p.id !== id);
      saveCustomPlaylists(custom);
      if (activeId.value === id) activeId.value = null;
      await refresh();
    } else {
      await api.deletePlaylist(id);
      if (activeId.value === id) activeId.value = null;
      await refresh();
    }
  }

  async function addMedia(id: number, media: MediaItem) {
    if (id < 0) {
      const custom = loadCustomPlaylists();
      const target = custom.find((p) => p.id === id);
      if (target) {
        target.tracks.push(media);
        target.updatedAt = Date.now();
        saveCustomPlaylists(custom);
        if (id === activeId.value) await loadTracks(id);
        await refresh();
      }
    } else {
      await api.addToPlaylist(id, media.id);
      if (id === activeId.value) await loadTracks(id);
      await refresh();
    }
  }

  async function removeMedia(id: number, media: MediaItem) {
    if (id < 0) {
      const custom = loadCustomPlaylists();
      const target = custom.find((p) => p.id === id);
      if (target) {
        target.tracks = target.tracks.filter((t) => t.path !== media.path && t.id !== media.id);
        target.updatedAt = Date.now();
        saveCustomPlaylists(custom);
        if (id === activeId.value) await loadTracks(id);
        await refresh();
      }
    } else {
      await api.removeFromPlaylist(id, media.id);
      if (id === activeId.value) await loadTracks(id);
      await refresh();
    }
  }

  async function reorder(id: number, ids: number[]) {
    if (id < 0) {
      const custom = loadCustomPlaylists();
      const target = custom.find((p) => p.id === id);
      if (target) {
        const reordered: MediaItem[] = [];
        for (const trackId of ids) {
          const item = target.tracks.find((t) => t.id === trackId);
          if (item) reordered.push(item);
        }
        target.tracks = reordered;
        target.updatedAt = Date.now();
        saveCustomPlaylists(custom);
        await loadTracks(id);
        await refresh();
      }
    } else {
      await api.reorderPlaylist(id, ids);
      await loadTracks(id);
      await refresh();
    }
  }

  async function play(id: number, index = 0) {
    if (id < 0) {
      const custom = loadCustomPlaylists().find((p) => p.id === id);
      if (custom && custom.tracks.length) {
        const normalizedTracks = custom.tracks.map((media) => {
          if (media.path.startsWith("http://") || media.path.startsWith("https://")) {
            return {
              ...media,
              path: normalizeMediaUrl(media.path),
            };
          } else if (media.path.startsWith("torrent://")) {
            const rawPath = decodeURIComponent(media.path.replace(/^torrent:\/\/[^/]+\//, ""));
            const archiveName = (custom.name || media.album || "").trim().replace(/\s+/g, "-");
            if (archiveName) {
              const upgradeUrl = normalizeMediaUrl(`https://archive.org/download/${encodeURIComponent(archiveName)}/${encodeURIComponent(rawPath)}`);
              return {
                ...media,
                path: upgradeUrl,
              };
            }
          }
          return media;
        });

        const target = normalizedTracks[index];
        if (target && target.path.startsWith("torrent://")) {
          throw new Error("This torrent track cannot be streamed directly because it does not have online web seeds. P2P streaming requires downloading the files first.");
        }

        const player = usePlayerStore();
        await player.playList(normalizedTracks, index, custom.name);
      }
    } else {
      await api.playPlaylist(id, index);
    }
  }

  async function exportPlaylist(id: number, name: string) {
    if (id < 0) {
      const custom = loadCustomPlaylists().find((p) => p.id === id);
      if (!custom || !custom.tracks.length) return null;
      let m3u = "#EXTM3U\n";
      for (const item of custom.tracks) {
        const durSec = Math.floor(item.durationMs / 1000);
        m3u += `#EXTINF:${durSec},${item.artist ? item.artist + " - " : ""}${item.title}\n`;
        m3u += `${item.path}\n`;
      }
      const blob = new Blob([m3u], { type: "audio/x-mpegurl" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${name || "playlist"}.m3u8`;
      a.click();
      URL.revokeObjectURL(url);
      return `${name}.m3u8`;
    }
    return api.exportPlaylist(id, name);
  }

  async function importPlaylist(name?: string) {
    const id = await api.importPlaylist(name);
    await refresh();
    if (id != null) await select(id);
    return id;
  }

  async function importM3uContent(name: string, content: string, sourceUrl?: string) {
    const defaultName = name.trim() || "Imported Playlist";
    const parsed = parseM3u(content, defaultName, sourceUrl);
    if (!parsed.items.length) {
      throw new Error("No playable tracks or streams found in this M3U.");
    }
    const id = -Math.floor(Date.now() + Math.random() * 1000);
    const finalName = name.trim() || parsed.title || "Imported Playlist";

    const custom = loadCustomPlaylists();
    custom.unshift({
      id,
      name: finalName,
      sourceUrl,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      tracks: parsed.items,
    });
    saveCustomPlaylists(custom);
    await refresh();
    await select(id);

    return {
      id,
      name: finalName,
      total: parsed.items.length,
      audioCount: parsed.audioCount,
      videoCount: parsed.videoCount,
    };
  }

  async function fetchPlaylistText(url: string): Promise<string> {
    // 1. Try native backend command (bypasses browser CORS completely)
    try {
      const nativeText = await api.fetchUrlText(url);
      if (nativeText && nativeText.trim()) return nativeText;
    } catch {
      // Continue to next fallbacks
    }

    // 2. Direct fetch (works if remote origin allows CORS)
    try {
      const response = await fetch(url, { headers: { Accept: "*/*" } });
      if (response.ok) {
        const text = await response.text();
        if (text && text.trim()) return text;
      }
    } catch {
      // CORS block or network error, proceed to proxies
    }

    // 3. Fallback: allorigins CORS proxy
    try {
      const proxyUrl = `https://api.allorigins.win/raw?url=${encodeURIComponent(url)}`;
      const response = await fetch(proxyUrl);
      if (response.ok) {
        const text = await response.text();
        if (text && text.trim()) return text;
      }
    } catch {
      // Continue
    }

    // 4. Fallback: corsproxy.io
    try {
      const proxyUrl = `https://corsproxy.io/?url=${encodeURIComponent(url)}`;
      const response = await fetch(proxyUrl);
      if (response.ok) {
        const text = await response.text();
        if (text && text.trim()) return text;
      }
    } catch {
      // Continue
    }

    throw new Error("Could not download playlist due to CORS network restriction. Please download the .m3u file and import using the 'Local File' tab.");
  }

  async function importM3uFromUrl(url: string, playlistName?: string) {
    const cleanUrl = url.trim();
    if (!cleanUrl) throw new Error("Please enter a valid URL.");

    const text = await fetchPlaylistText(cleanUrl);

    let name = playlistName?.trim();
    if (!name) {
      const filePart = cleanUrl.split("/").pop()?.split("?")[0] || "Online Playlist";
      name = decodeURIComponent(filePart)
        .replace(/\.[^/.]+$/, "")
        .replace(/_vbr$|_hq$|_m3u8?$/i, "")
        .replace(/[-_]/g, " ")
        .replace(/\b\w/g, (c) => c.toUpperCase());
    }

    return importM3uContent(name, text, cleanUrl);
  }

  async function importM3uFromFile(file: File, playlistName?: string) {
    const text = await file.text();
    let name = playlistName?.trim();
    if (!name) {
      name = file.name.replace(/\.[^/.]+$/, "").replace(/[-_]/g, " ");
    }
    return importM3uContent(name, text);
  }

  async function importTorrentFile(file: File, playlistName?: string) {
    const buffer = await file.arrayBuffer();
    const result = await parseTorrent(buffer);
    if (!result.items.length) {
      throw new Error("No playable audio or video files found inside this .torrent.");
    }

    const id = -Math.floor(Date.now() + Math.random() * 1000);
    const finalName = playlistName?.trim() || result.name || "Torrent Playlist";

    const custom = loadCustomPlaylists();
    custom.unshift({
      id,
      name: finalName,
      sourceUrl: result.magnetUri,
      coverPath: result.thumbnail ?? null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      tracks: result.items,
    });
    saveCustomPlaylists(custom);
    await refresh();
    await select(id);

    return {
      id,
      name: finalName,
      total: result.items.length,
      audioCount: result.audioCount,
      videoCount: result.videoCount,
      thumbnail: result.thumbnail,
      infoHash: result.infoHash,
    };
  }

  async function importTorrentTracks(
    playlistName: string,
    tracks: MediaItem[],
    magnetUri?: string,
    coverPath?: string | null,
  ) {
    if (!tracks.length) {
      throw new Error("Please select at least one media file to import.");
    }
    const id = -Math.floor(Date.now() + Math.random() * 1000);
    const finalName = playlistName.trim() || "Torrent Playlist";

    const custom = loadCustomPlaylists();
    custom.unshift({
      id,
      name: finalName,
      sourceUrl: magnetUri,
      coverPath: coverPath ?? null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      tracks,
    });
    saveCustomPlaylists(custom);
    await refresh();
    await select(id);

    return {
      id,
      name: finalName,
      total: tracks.length,
      audioCount: tracks.filter((t) => t.kind === "audio").length,
      videoCount: tracks.filter((t) => t.kind === "video").length,
    };
  }

  function clearLocal() {
    playlists.value = [];
    tracks.value = [];
    activeId.value = null;
    privateVisible.value = false;
  }

  async function setLocked(id: number, locked: boolean) {
    await api.setPlaylistLocked(id, locked);
    if (locked && activeId.value === id) activeId.value = null;
    await refresh();
  }

  async function revealPrivate(password?: string) {
    privateVisible.value = await api.revealPrivatePlaylists(password);
    await refresh();
    return privateVisible.value;
  }

  async function hidePrivate() {
    await api.hidePrivatePlaylists();
    privateVisible.value = false;
    await refresh();
  }

  return {
    playlists,
    activeId,
    tracks,
    loading,
    privateVisible,
    active,
    clearLocal,
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
    setLocked,
    revealPrivate,
    hidePrivate,
    exportPlaylist,
    importPlaylist,
    importM3uContent,
    importM3uFromUrl,
    importM3uFromFile,
    importTorrentFile,
    importTorrentTracks,
  };
});
