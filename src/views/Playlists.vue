<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  ListMusic, Plus, Pencil, Trash2, Download, Upload, Play, X, ArrowUp, ArrowDown,
  Music2, MoreVertical, ListPlus, Heart, Film, Captions, Info, Tv, Globe, Link, FileText, Loader2,
  CheckSquare, Square, Filter, Image as ImageIcon
} from "@lucide/vue";
import { usePlaylistStore } from "../stores/playlist";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useMediaProperties } from "../composables/useMediaProperties";
import { useStoryboardTask } from "../composables/useStoryboardTask";
import { useSubtitleAvailability } from "../composables/useSubtitleAvailability";
import { formatDuration, formatBytes } from "../types/media";
import { parseTorrent } from "../utils/torrent";
import type { ParsedTorrentResult } from "../utils/torrent";
import type { MediaItem } from "../types/media";
import ContextMenu from "../components/ContextMenu.vue";
import type { ContextMenuItem } from "../types/contextMenu";

const playlists = usePlaylistStore();
const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();
const picker = usePlaylistPicker();
const properties = useMediaProperties();
const storyboard = useStoryboardTask();
const subtitles = useSubtitleAvailability();

watch(() => playlists.tracks, (tracks) => {
  void subtitles.checkMany(tracks.map((entry) => entry.media));
}, { immediate: true });

function hasCaption(item: MediaItem): boolean {
  return item.kind === "video" && subtitles.has(item.id);
}

const newName = ref("");
const renamingId = ref<number | null>(null);
const renameValue = ref("");
const pickerOpen = ref(false);
const pickerSearch = ref("");

// Import Modal State
const showImportModal = ref(false);
const importTab = ref<"url" | "file">("url");
const importUrl = ref("");
const importName = ref("");
const importLoading = ref(false);
const selectedFile = ref<File | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);

// Torrent Browser state
const torrentResult = ref<ParsedTorrentResult | null>(null);
const selectedTorrentFileIndices = ref<Set<number>>(new Set());
const activeTorrentExtFilter = ref<string>("all");

const audioTracksCount = computed(() => playlists.tracks.filter((t) => t.media.kind === "audio").length);
const videoTracksCount = computed(() => playlists.tracks.filter((t) => t.media.kind === "video").length);

const filteredTorrentItems = computed(() => {
  if (!torrentResult.value) return [];
  const filter = activeTorrentExtFilter.value;
  if (filter === "all") return torrentResult.value.items;
  if (filter === "audio") return torrentResult.value.items.filter((i) => i.kind === "audio");
  if (filter === "video") return torrentResult.value.items.filter((i) => i.kind === "video");
  return torrentResult.value.items.filter((i) => i.ext === filter);
});

const selectedTorrentCount = computed(() => selectedTorrentFileIndices.value.size);

const selectedTorrentSizeBytes = computed(() => {
  if (!torrentResult.value) return 0;
  return torrentResult.value.items
    .filter((i) => selectedTorrentFileIndices.value.has(i.fileIndex))
    .reduce((acc, curr) => acc + (curr.sizeBytes || 0), 0);
});

const isAllVisibleTorrentSelected = computed(() => {
  if (!filteredTorrentItems.value.length) return false;
  return filteredTorrentItems.value.every((i) => selectedTorrentFileIndices.value.has(i.fileIndex));
});

function toggleTorrentFileIndex(fileIndex: number) {
  const next = new Set(selectedTorrentFileIndices.value);
  if (next.has(fileIndex)) {
    next.delete(fileIndex);
  } else {
    next.add(fileIndex);
  }
  selectedTorrentFileIndices.value = next;
}

function toggleSelectVisibleTorrent() {
  const next = new Set(selectedTorrentFileIndices.value);
  if (isAllVisibleTorrentSelected.value) {
    for (const item of filteredTorrentItems.value) {
      next.delete(item.fileIndex);
    }
  } else {
    for (const item of filteredTorrentItems.value) {
      next.add(item.fileIndex);
    }
  }
  selectedTorrentFileIndices.value = next;
}

function selectOnlyFilter(filter: string) {
  activeTorrentExtFilter.value = filter;
  if (!torrentResult.value) return;
  const next = new Set<number>();
  for (const item of torrentResult.value.items) {
    if (filter === "all") {
      next.add(item.fileIndex);
    } else if (filter === "audio" && item.kind === "audio") {
      next.add(item.fileIndex);
    } else if (filter === "video" && item.kind === "video") {
      next.add(item.fileIndex);
    } else if (item.ext === filter) {
      next.add(item.fileIndex);
    }
  }
  selectedTorrentFileIndices.value = next;
}

function selectAllTorrent() {
  if (!torrentResult.value) return;
  selectedTorrentFileIndices.value = new Set(torrentResult.value.items.map((i) => i.fileIndex));
}

function deselectAllTorrent() {
  selectedTorrentFileIndices.value = new Set();
}

function openImportModal() {
  importUrl.value = "";
  importName.value = "";
  selectedFile.value = null;
  torrentResult.value = null;
  selectedTorrentFileIndices.value = new Set();
  activeTorrentExtFilter.value = "all";
  importLoading.value = false;
  showImportModal.value = true;
}

async function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files && input.files[0]) {
    selectedFile.value = input.files[0];
    torrentResult.value = null;
    selectedTorrentFileIndices.value = new Set();
    activeTorrentExtFilter.value = "all";
    const name = selectedFile.value.name.replace(/\.[^/.]+$/, "").replace(/[-_]/g, " ");
    if (!importName.value) {
      importName.value = name;
    }
    if (selectedFile.value.name.toLowerCase().endsWith(".torrent")) {
      try {
        importLoading.value = true;
        const buffer = await selectedFile.value.arrayBuffer();
        const parsed = await parseTorrent(buffer);
        torrentResult.value = parsed;
        // Select all media files by default
        selectedTorrentFileIndices.value = new Set(parsed.items.map((i) => i.fileIndex));
        if (!importName.value || importName.value === name) {
          importName.value = parsed.name;
        }
      } catch (err) {
        toast.error(String(err), "Torrent parse error");
      } finally {
        importLoading.value = false;
      }
    }
  }
}

async function handleImportUrl() {
  const url = importUrl.value.trim();
  if (!url) {
    toast.error("Please enter an M3U playlist URL");
    return;
  }
  importLoading.value = true;
  try {
    const res = await playlists.importM3uFromUrl(url, importName.value);
    toast.success(`Imported “${res.name}” (${res.total} tracks · ${res.audioCount} Audio, ${res.videoCount} Video).`);
    showImportModal.value = false;
  } catch (err) {
    toast.error(String(err), "Import failed");
  } finally {
    importLoading.value = false;
  }
}

async function handleImportFile() {
  if (!selectedFile.value) {
    toast.error("Please select an .m3u, .m3u8, or .torrent file");
    return;
  }
  importLoading.value = true;
  try {
    const isTorrent = selectedFile.value.name.toLowerCase().endsWith(".torrent");
    if (isTorrent) {
      if (!torrentResult.value) {
        throw new Error("Torrent could not be parsed.");
      }
      const selectedTracks = torrentResult.value.items.filter((i) =>
        selectedTorrentFileIndices.value.has(i.fileIndex)
      );
      if (!selectedTracks.length) {
        toast.error("Please select at least 1 file to import.");
        importLoading.value = false;
        return;
      }
      const res = await playlists.importTorrentTracks(
        importName.value || torrentResult.value.name,
        selectedTracks,
        torrentResult.value.magnetUri,
        torrentResult.value.thumbnail
      );
      toast.success(
        `Imported torrent “${res.name}” (${res.total} media files · ${res.audioCount} Audio, ${res.videoCount} Video).`
      );
    } else {
      const res = await playlists.importM3uFromFile(selectedFile.value, importName.value);
      toast.success(`Imported “${res.name}” (${res.total} tracks · ${res.audioCount} Audio, ${res.videoCount} Video).`);
    }
    showImportModal.value = false;
  } catch (err) {
    toast.error(String(err), "Import failed");
  } finally {
    importLoading.value = false;
  }
}

const pickerItems = computed(() => {
  const query = pickerSearch.value.trim().toLowerCase();
  return library.items.filter((item) => !query || `${item.title} ${item.artist ?? ""}`.toLowerCase().includes(query));
});

async function create() {
  if (!newName.value.trim()) return;
  try {
    await playlists.create(newName.value);
    newName.value = "";
    toast.success("Playlist created.");
  } catch (error) {
    toast.error(String(error), "Could not create playlist");
  }
}

async function rename(id: number) {
  if (!renameValue.value.trim()) return;
  try {
    await playlists.rename(id, renameValue.value);
    renamingId.value = null;
  } catch (error) {
    toast.error(String(error), "Could not rename playlist");
  }
}

async function remove(id: number) {
  try {
    await playlists.remove(id);
    toast.info("Playlist deleted.");
  } catch (error) {
    toast.error(String(error), "Could not delete playlist");
  }
}

async function play(index: number) {
  if (!playlists.active) return;
  try {
    await playlists.play(playlists.active.id, index);
  } catch (error) {
    toast.error(String(error), "Playback failed");
  }
}

async function removeTrack(media: MediaItem) {
  if (!playlists.active) return;
  try {
    await playlists.removeMedia(playlists.active.id, media);
  } catch (error) {
    toast.error(String(error), "Could not remove track");
  }
}

async function move(index: number, delta: number) {
  if (!playlists.active) return;
  const target = index + delta;
  const ids = playlists.tracks.map((entry) => entry.media.id);
  if (target < 0 || target >= ids.length) return;
  [ids[index], ids[target]] = [ids[target], ids[index]];
  try {
    await playlists.reorder(playlists.active.id, ids);
  } catch (error) {
    toast.error(String(error), "Could not reorder tracks");
  }
}

async function addTrack(media: MediaItem) {
  if (!playlists.active) return;
  try {
    await playlists.addMedia(playlists.active.id, media);
    toast.success(`Added “${media.title}”.`);
  } catch (error) {
    toast.error(String(error), "Could not add track");
  }
}

async function exportActive() {
  if (!playlists.active) return;
  try {
    const path = await playlists.exportPlaylist(playlists.active.id, playlists.active.name);
    if (path) toast.success(`Exported to ${path}`, "M3U playlist");
    else toast.info("Export cancelled.");
  } catch (error) {
    toast.error(String(error), "Could not export playlist");
  }
}

async function exportOne(id: number, name: string) {
  try {
    const path = await playlists.exportPlaylist(id, name);
    if (path) toast.success(`Exported to ${path}`, "M3U playlist");
    else toast.info("Export cancelled.");
  } catch (error) {
    toast.error(String(error), "Could not export playlist");
  }
}

type MenuState =
  | { kind: "playlist"; x: number; y: number; id: number; name: string }
  | { kind: "track"; x: number; y: number; index: number };

const menu = ref<MenuState | null>(null);
const subtitleAvailable = ref(false);

function openPlaylistMenu(event: MouseEvent, playlist: { id: number; name: string }) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = { kind: "playlist", x: event.clientX, y: event.clientY, id: playlist.id, name: playlist.name };
}

function openTrackMenu(event: MouseEvent, index: number) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = { kind: "track", x: event.clientX, y: event.clientY, index };
  subtitleAvailable.value = false;
  const item = playlists.tracks[index]?.media;
  if (item?.kind === "video") {
    void storyboard.check(item.id);
    subtitleAvailable.value = subtitles.get(item.id) ?? false;
    void subtitles.check(item.id).then((value) => {
      subtitleAvailable.value = value;
    });
  }
}

const menuDefinition = computed<{ items: ContextMenuItem[]; actions: string[] }>(() => {
  const current = menu.value;
  if (!current) return { items: [], actions: [] };
  if (current.kind === "playlist") {
    return {
      items: [
        { label: "Select playlist", icon: ListMusic },
        { label: "Rename", icon: Pencil },
        { label: "Export M3U", icon: Download },
        { label: "Delete", icon: Trash2, danger: true },
      ],
      actions: ["select", "rename", "export", "delete"],
    };
  }
  const items: ContextMenuItem[] = [
    { label: "Play now", icon: Play },
    { label: "Play next", icon: ListPlus },
    { label: "Add to queue", icon: ListMusic },
    { label: "Toggle favorite", icon: Heart },
    { label: "Add to playlist…", icon: Plus },
    { label: "Properties…", icon: Info },
  ];
  const actions = ["play", "next", "queue", "favorite", "playlist", "properties"];
  const item = playlists.tracks[current.index]?.media;
  if (item?.kind === "video") {
    const task = storyboard.status(item.id);
    if (task) {
      items.push({ label: `Generating… ${task.done}/${task.total}`, icon: Film, disabled: true });
      actions.push("storyboard");
    } else if (!storyboard.isReady(item.id)) {
      items.push({ label: "Storyboard", icon: Film });
      actions.push("storyboard");
    }
    items.push({ label: "Load subtitle file…", icon: Captions, disabled: subtitleAvailable.value });
    actions.push("subtitle");
  }
  items.push({ label: "Move up", icon: ArrowUp, disabled: current.index === 0 });
  items.push({ label: "Move down", icon: ArrowDown, disabled: current.index === playlists.tracks.length - 1 });
  items.push({ label: "Remove from playlist", icon: X, danger: true });
  actions.push("up", "down", "remove");
  return { items, actions };
});

async function onMenuSelect(index: number) {
  const current = menu.value;
  if (!current) return;
  const action = menuDefinition.value.actions[index];
  if (current.kind === "playlist") {
    switch (action) {
      case "select":
        playlists.select(current.id);
        break;
      case "rename":
        renamingId.value = current.id;
        renameValue.value = current.name;
        break;
      case "export":
        await exportOne(current.id, current.name);
        break;
      case "delete":
        await remove(current.id);
        break;
    }
    return;
  }
  const entry = playlists.tracks[current.index];
  if (!entry) return;
  switch (action) {
    case "play":
      await play(current.index);
      break;
    case "next":
      try {
        await player.queueNext(entry.media);
        toast.success(`Playing next: “${entry.media.title}”.`);
      } catch (error) {
        toast.error(String(error), "Could not queue item");
      }
      break;
    case "queue":
      try {
        await player.queueAdd(entry.media);
        toast.success(`Queued “${entry.media.title}”.`);
      } catch (error) {
        toast.error(String(error), "Could not queue item");
      }
      break;
    case "favorite":
      try {
        await library.toggleFavorite(entry.media);
      } catch (error) {
        toast.error(String(error), "Could not update favorite");
      }
      break;
    case "playlist":
      picker.askAddToPlaylist(entry.media);
      break;
    case "properties":
      properties.showProperties(entry.media);
      break;
    case "storyboard":
      try {
        await storyboard.generate(entry.media.id);
        player.reloadStoryboard(entry.media.id);
        toast.success(`Storyboard ready for “${entry.media.title}”.`);
      } catch (error) {
        toast.error(String(error), "Could not generate storyboard");
      }
      break;
    case "subtitle":
      await player.openSubtitle();
      break;
    case "up":
      await move(current.index, -1);
      break;
    case "down":
      await move(current.index, 1);
      break;
    case "remove":
      await removeTrack(entry.media);
      break;
  }
}
</script>

<template>
  <div class="flex h-full min-h-0 gap-5 px-5 py-4">
    <div class="flex w-[260px] shrink-0 flex-col gap-3">
      <div class="flex items-center gap-2">
        <span class="flex h-9 w-9 items-center justify-center rounded-xl bg-linear-to-br from-purple to-blue text-white"><ListMusic :size="18" /></span>
        <h1 class="text-lg font-bold text-ink">Playlists</h1>
      </div>
      <div class="flex gap-2">
        <input v-model="newName" class="min-w-0 flex-1 rounded-lg border border-line bg-panel px-3 text-[12px] text-ink outline-none placeholder:text-faint focus:border-purple/60" placeholder="New playlist name" @keydown.enter="create" />
        <button type="button" class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-purple text-white hover:brightness-110" title="Create playlist" @click="create"><Plus :size="17" /></button>
      </div>
      <div class="flex gap-2">
        <button type="button" class="playlist-action flex-1" title="Import M3U / URL" @click="openImportModal">
          <Upload :size="13" /> Import M3U
        </button>
        <button type="button" class="playlist-action flex-1" :disabled="!playlists.active" @click="exportActive"><Download :size="13" /> Export</button>
      </div>
      <div class="min-h-0 flex-1 space-y-1.5 overflow-y-auto pr-1">
        <div
          v-for="playlist in playlists.playlists"
          :key="playlist.id"
          class="group flex items-center gap-2 rounded-xl border px-3 py-2.5 transition-colors"
          :class="playlists.activeId === playlist.id ? 'border-purple/50 bg-purple/15' : 'border-line bg-panel hover:bg-hover'"
          @contextmenu.prevent="openPlaylistMenu($event, playlist)"
        >
          <button type="button" class="min-w-0 flex-1 text-left flex items-center gap-2"
            @click="playlists.select(playlist.id)">
            <img v-if="playlist.coverPath" :src="playlist.coverPath"
              class="h-7 w-7 rounded-md object-cover border border-line shrink-0 shadow-xs" alt="Cover" />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5 truncate text-[12px] font-semibold text-ink">
                <Globe v-if="playlist.id < 0 && !playlist.coverPath" :size="12" class="shrink-0 text-purple"
                  title="Online / Torrent Playlist" />
                <span class="truncate">{{ playlist.name }}</span>
              </div>
              <div class="text-[10px] text-faint">{{ playlist.trackCount }} track{{ playlist.trackCount === 1 ? "" : "s"
                }}</div>
            </div>
          </button>
          <button type="button" class="action-icon" title="Rename" @click="renamingId = playlist.id; renameValue = playlist.name"><Pencil :size="12" /></button>
          <button type="button" class="action-icon hover:!text-[#ff5b8d]" title="Delete" @click="remove(playlist.id)"><Trash2 :size="12" /></button>
          <button type="button" class="action-icon" title="More actions" @click="openPlaylistMenu($event, playlist)"><MoreVertical :size="12" /></button>
        </div>
        <div v-if="!playlists.playlists.length" class="mt-8 text-center text-[11px] leading-relaxed text-faint">No
          playlists yet. Create one above or import an M3U playlist.</div>
      </div>
    </div>

    <div class="su-panel flex min-w-0 flex-1 flex-col overflow-hidden">
      <template v-if="playlists.active">
        <div class="flex h-14 shrink-0 items-center gap-3 border-b border-line px-5">
          <div v-if="renamingId === playlists.active.id" class="flex flex-1 gap-2">
            <input v-model="renameValue" class="min-w-0 flex-1 rounded-lg border border-line bg-elevated px-3 text-[13px] text-ink outline-none focus:border-purple/60" @keydown.enter="rename(playlists.active!.id)" />
            <button type="button" class="playlist-action" @click="rename(playlists.active!.id)">Save</button>
          </div>
          <template v-else>
            <div class="min-w-0 flex-1 flex items-center gap-3">
              <img v-if="playlists.active.coverPath" :src="playlists.active.coverPath"
                class="h-10 w-10 rounded-lg object-cover border border-line shrink-0 shadow-sm" alt="Cover" />
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 truncate text-[15px] font-semibold text-ink">
                  <Globe v-if="playlists.active.id < 0 && !playlists.active.coverPath" :size="15"
                    class="shrink-0 text-purple" title="Online / Torrent Playlist" />
                  <span class="truncate">{{ playlists.active.name }}</span>
                </div>
                <div class="text-[11px] text-faint">
                  {{ playlists.tracks.length }} track{{ playlists.tracks.length === 1 ? "" : "s" }}
                  <span v-if="playlists.tracks.length > 0">
                    · <span class="text-purple font-medium">{{ audioTracksCount }} Audio (Music)</span>, <span
                      class="text-sky-400 font-medium">{{ videoTracksCount }} Video</span>
                  </span>
                </div>
              </div>
            </div>
            <button type="button" class="playlist-action" @click="pickerOpen = true"><Plus :size="13" /> Add media</button>
          </template>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto">
          <div
            class="grid grid-cols-[32px_1fr_120px_90px] gap-2 border-b border-line px-5 py-2 text-[10px] font-semibold uppercase tracking-wider text-faint">
            <span>#</span><span>Track & Kind</span><span>Category / Album</span><span class="text-right">Duration</span>
          </div>
          <div
            v-for="(entry, index) in playlists.tracks"
            :key="`${entry.media.id}-${index}`"
            class="group grid grid-cols-[32px_1fr_120px_90px] items-center gap-2 border-b border-line/60 px-5 py-2.5 text-[12px] hover:bg-hover/60"
            @contextmenu.prevent="openTrackMenu($event, index)"
          >
            <button type="button" class="flex h-7 w-7 items-center justify-center rounded-md text-faint hover:bg-purple hover:text-white" title="Play" @click="play(index)"><Play :size="12" fill="currentColor" /></button>
            <div class="flex min-w-0 items-center gap-2">
              <!-- Audio vs Video Kind Badge -->
              <span v-if="entry.media.kind === 'video'"
                class="flex shrink-0 items-center gap-1 rounded bg-sky-500/15 px-1.5 py-0.5 text-[9px] font-semibold text-sky-400"
                title="Video / Live TV stream">
                <Tv :size="10" />
                <span>Video</span>
              </span>
              <span v-else
                class="flex shrink-0 items-center gap-1 rounded bg-purple/15 px-1.5 py-0.5 text-[9px] font-semibold text-purple"
                title="Audio / Music stream">
                <Music2 :size="10" />
                <span>Audio</span>
              </span>

              <span class="truncate font-medium text-ink">{{ entry.media.title }}</span>
              <span v-if="entry.media.artist" class="truncate text-[11px] text-faint">— {{ entry.media.artist }}</span>

              <!-- Companion Thumbnail badge if present -->
              <span v-if="(entry.media as any).companionThumbnail"
                class="flex shrink-0 items-center gap-1 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[9px] font-medium text-emerald-400"
                :title="`Companion Artwork: ${(entry.media as any).companionThumbnail}`">
                <ImageIcon :size="9" />
                <span class="max-w-[70px] truncate">{{ (entry.media as any).companionThumbnail }}</span>
              </span>

              <span v-if="hasCaption(entry.media)"
                class="shrink-0 rounded bg-purple/20 px-1 text-[9px] font-bold leading-4 text-purple"
                title="Subtitles available">CC</span>
              <span class="ml-auto flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                <button type="button" class="action-icon" title="Move up" @click="move(index, -1)"><ArrowUp :size="12" /></button>
                <button type="button" class="action-icon" title="Move down" @click="move(index, 1)"><ArrowDown :size="12" /></button>
                <button type="button" class="action-icon hover:!text-[#ff5b8d]" title="Remove" @click="removeTrack(entry.media)"><X :size="12" /></button>
                <button type="button" class="action-icon" title="More actions" @click.stop="openTrackMenu($event, index)"><MoreVertical :size="12" /></button>
              </span>
            </div>
            <span class="truncate text-faint text-[11px]">{{ entry.media.album || entry.media.artist || "—" }}</span>
            <span class="text-right tabular-nums text-faint text-[11px]">{{ entry.media.durationMs > 0 ?
              formatDuration(entry.media.durationMs) : (entry.media.sizeBytes > 0 ? formatBytes(entry.media.sizeBytes) :
              "—") }}</span>
          </div>
          <div v-if="!playlists.tracks.length" class="py-16 text-center text-[12px] text-faint">This playlist is empty.
            Add media or import an M3U file.</div>
        </div>
      </template>
      <div v-else class="flex flex-1 flex-col items-center justify-center text-center">
        <ListMusic :size="38" class="mb-3 text-faint" />
        <p class="text-sm font-medium text-muted">No playlist selected</p>
        <p class="mt-1 text-xs text-faint">Create or select a playlist to see its tracks.</p>
      </div>
    </div>

    <!-- Add media picker modal -->
    <div v-if="pickerOpen" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" @click.self="pickerOpen = false">
      <div class="su-panel flex max-h-[70vh] w-[520px] flex-col overflow-hidden p-5">
        <div class="mb-4 flex items-center gap-3">
          <h2 class="flex-1 text-base font-semibold text-ink">Add media to {{ playlists.active?.name }}</h2>
          <button type="button" class="action-icon" @click="pickerOpen = false"><X :size="16" /></button>
        </div>
        <input v-model="pickerSearch" class="mb-3 rounded-lg border border-line bg-elevated px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60" placeholder="Search your library…" />
        <div class="min-h-0 flex-1 space-y-1 overflow-y-auto">
          <button v-for="item in pickerItems" :key="item.id" type="button" class="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left hover:bg-hover" @click="addTrack(item); pickerOpen = false">
            <Music2 :size="14" class="text-faint" />
            <span class="min-w-0 flex-1 truncate text-[12px] text-ink">{{ item.title }}</span>
            <span class="truncate text-[10px] text-faint">{{ item.artist || 'Unknown artist' }}</span>
            <Plus :size="14" class="text-purple-soft" />
          </button>
        </div>
      </div>
    </div>

    <!-- M3U / Torrent Playlist Import Modal -->
    <div v-if="showImportModal" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      @click.self="showImportModal = false">
      <div
        class="su-panel flex max-h-[90vh] flex-col overflow-hidden p-6 rounded-2xl shadow-2xl border border-line transition-all"
        :class="torrentResult ? 'w-[640px]' : 'w-[540px]'">
        <div class="mb-3.5 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="flex h-8 w-8 items-center justify-center rounded-lg bg-purple/20 text-purple">
              <Upload :size="16" />
            </span>
            <h2 class="text-base font-semibold text-ink">Import Playlist (.m3u / .m3u8 / .torrent)</h2>
          </div>
          <button type="button" class="action-icon" @click="showImportModal = false">
            <X :size="16" />
          </button>
        </div>

        <p v-if="!torrentResult" class="mb-4 text-[12px] text-faint leading-relaxed">
          Online ya local M3U playlist ya <strong>.torrent</strong> file import karein. App automatically check karega
          ki kaunsi file <strong>Audio</strong> hai aur kaunsi <strong>Video</strong>, aur companion thumbnails bhi load
          karega.
        </p>

        <!-- Tabs (hide when browsing torrent to maximize vertical space) -->
        <div v-if="!torrentResult" class="mb-4 flex rounded-xl border border-line bg-secondary/40 p-1">
          <button type="button"
            class="flex-1 rounded-lg py-1.5 text-[12px] font-semibold transition-all flex items-center justify-center gap-1.5"
            :class="importTab === 'url' ? 'bg-purple text-white shadow-sm' : 'text-faint hover:text-ink'"
            @click="importTab = 'url'">
            <Link :size="13" /> Online URL (Web Link)
          </button>
          <button type="button"
            class="flex-1 rounded-lg py-1.5 text-[12px] font-semibold transition-all flex items-center justify-center gap-1.5"
            :class="importTab === 'file' ? 'bg-purple text-white shadow-sm' : 'text-faint hover:text-ink'"
            @click="importTab = 'file'">
            <FileText :size="13" /> Local File (.m3u / .torrent)
          </button>
        </div>

        <!-- URL Form -->
        <div v-if="importTab === 'url' && !torrentResult" class="space-y-3.5">
          <div>
            <label class="block text-[11px] font-semibold text-muted mb-1">M3U Playlist URL</label>
            <input v-model="importUrl"
              class="w-full rounded-xl border border-line bg-elevated px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60 placeholder:text-faint"
              placeholder="https://archive.org/.../playlist_vbr.m3u" :disabled="importLoading"
              @keydown.enter="handleImportUrl" />
          </div>

          <div>
            <label class="block text-[11px] font-semibold text-muted mb-1">Playlist Name (Optional)</label>
            <input v-model="importName"
              class="w-full rounded-xl border border-line bg-elevated px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60 placeholder:text-faint"
              placeholder="e.g. Non Stop Party Jukebox 2024" :disabled="importLoading"
              @keydown.enter="handleImportUrl" />
          </div>

          <div class="flex justify-end gap-2 pt-2">
            <button type="button" class="playlist-action" :disabled="importLoading" @click="showImportModal = false">
              Cancel
            </button>
            <button type="button"
              class="flex items-center gap-1.5 rounded-xl bg-purple px-4 py-2 text-[12px] font-semibold text-white hover:brightness-110 disabled:opacity-50"
              :disabled="importLoading || !importUrl.trim()" @click="handleImportUrl">
              <Loader2 v-if="importLoading" :size="14" class="animate-spin" />
              <span>{{ importLoading ? 'Parsing tracks…' : 'Import Playlist' }}</span>
            </button>
          </div>
        </div>

        <!-- File Form -->
        <div v-else class="space-y-3">
          <!-- Hidden file input -->
          <input ref="fileInput" type="file" accept=".m3u,.m3u8,.torrent" class="hidden" @change="onFileChange" />

          <!-- Case 1: Torrent File Browser & Selection -->
          <template v-if="torrentResult">
            <!-- Torrent Info Header Card -->
            <div class="flex items-center gap-3 rounded-xl border border-line bg-elevated/40 p-3">
              <img v-if="torrentResult.thumbnail" :src="torrentResult.thumbnail"
                class="h-12 w-12 rounded-lg object-cover border border-line shrink-0 shadow-sm" alt="Cover Art" />
              <div v-else
                class="flex h-12 w-12 items-center justify-center rounded-lg bg-purple/15 text-purple shrink-0">
                <FileText :size="22" />
              </div>
              <div class="min-w-0 flex-1">
                <div class="text-[13px] font-semibold text-ink truncate">{{ torrentResult.name }}</div>
                <div class="text-[11px] text-faint mt-0.5 flex flex-wrap items-center gap-2">
                  <span>{{ formatBytes(torrentResult.totalSizeBytes) }}</span>
                  <span>•</span>
                  <span class="text-purple font-medium">{{ torrentResult.audioCount }} Audio</span>
                  <span>•</span>
                  <span class="text-sky-400 font-medium">{{ torrentResult.videoCount }} Video</span>
                  <span v-if="torrentResult.hasWebSeeds" class="text-emerald-400 font-medium flex items-center gap-0.5">
                    • <Globe :size="10" /> Online Stream Ready
                  </span>
                  <span v-else class="text-amber-400 font-medium flex items-center gap-0.5">
                    • P2P
                  </span>
                  <span v-if="torrentResult.thumbnail" class="text-emerald-400 font-medium flex items-center gap-0.5">
                    • <ImageIcon :size="10" /> Cover Art
                  </span>
                </div>
              </div>
              <button type="button" class="playlist-action text-[11px] shrink-0" @click="fileInput?.click()">
                Change file
              </button>
            </div>

            <!-- Filter & Quick Select Chips -->
            <div>
              <div class="flex items-center justify-between mb-1.5">
                <span class="text-[11px] font-semibold text-muted flex items-center gap-1">
                  <Filter :size="12" /> Filter & Select:
                </span>
                <div class="flex items-center gap-2 text-[10px]">
                  <button type="button" class="text-purple hover:underline font-semibold" @click="selectAllTorrent">
                    Select All
                  </button>
                  <span class="text-faint">|</span>
                  <button type="button" class="text-faint hover:text-ink font-semibold" @click="deselectAllTorrent">
                    Clear Selection
                  </button>
                </div>
              </div>

              <div class="flex flex-wrap gap-1.5">
                <!-- All button -->
                <button type="button" class="rounded-lg px-2.5 py-1 text-[11px] font-medium transition-colors border"
                  :class="activeTorrentExtFilter === 'all'
                    ? 'bg-purple text-white border-purple shadow-xs'
                    : 'bg-elevated text-muted border-line hover:border-purple/40'" @click="selectOnlyFilter('all')">
                  All ({{ torrentResult.items.length }})
                </button>

                <!-- Audio button if any -->
                <button v-if="torrentResult.audioCount > 0" type="button"
                  class="rounded-lg px-2.5 py-1 text-[11px] font-medium transition-colors border flex items-center gap-1"
                  :class="activeTorrentExtFilter === 'audio'
                    ? 'bg-purple text-white border-purple shadow-xs'
                    : 'bg-elevated text-purple border-line hover:border-purple/40'" @click="selectOnlyFilter('audio')">
                  <Music2 :size="11" />
                  Audio ({{ torrentResult.audioCount }})
                </button>

                <!-- Video button if any -->
                <button v-if="torrentResult.videoCount > 0" type="button"
                  class="rounded-lg px-2.5 py-1 text-[11px] font-medium transition-colors border flex items-center gap-1"
                  :class="activeTorrentExtFilter === 'video'
                    ? 'bg-sky-500 text-white border-sky-500 shadow-xs'
                    : 'bg-elevated text-sky-400 border-line hover:border-sky-400/40'"
                  @click="selectOnlyFilter('video')">
                  <Tv :size="11" />
                  Video ({{ torrentResult.videoCount }})
                </button>

                <!-- Specific extension buttons: .mp3, .wav, .flac, .mp4, etc. -->
                <button v-for="ext in torrentResult.allExtensions" :key="ext" type="button"
                  class="rounded-lg px-2 py-1 text-[11px] font-mono uppercase font-semibold transition-colors border"
                  :class="activeTorrentExtFilter === ext
                    ? 'bg-purple text-white border-purple shadow-xs'
                    : 'bg-elevated text-faint border-line hover:border-purple/40 hover:text-ink'"
                  @click="selectOnlyFilter(ext)">
                  .{{ ext }}
                </button>
              </div>
            </div>

            <!-- Selection status & Select-all toggle bar -->
            <div
              class="flex items-center justify-between rounded-lg bg-panel border border-line px-3 py-1.5 text-[11px]">
              <button type="button" class="flex items-center gap-1.5 text-muted hover:text-ink font-medium select-none"
                @click="toggleSelectVisibleTorrent">
                <CheckSquare v-if="isAllVisibleTorrentSelected" :size="14" class="text-purple" />
                <Square v-else :size="14" class="text-faint" />
                <span>Toggle Visible</span>
              </button>
              <div class="text-[11px] text-faint">
                <span class="font-semibold text-ink">{{ selectedTorrentCount }}</span> of {{ torrentResult.items.length
                }} selected
                <span v-if="selectedTorrentCount > 0" class="text-purple font-medium">({{
                  formatBytes(selectedTorrentSizeBytes) }})</span>
              </div>
            </div>

            <!-- Scrollable Torrent File List -->
            <div
              class="min-h-0 max-h-[220px] overflow-y-auto rounded-xl border border-line bg-panel divide-y divide-line/60">
              <div v-for="item in filteredTorrentItems" :key="item.fileIndex"
                class="flex items-center gap-2.5 px-3 py-2 text-[12px] hover:bg-hover/60 cursor-pointer select-none transition-colors"
                :class="selectedTorrentFileIndices.has(item.fileIndex) ? 'bg-purple/5' : ''"
                @click="toggleTorrentFileIndex(item.fileIndex)">
                <!-- Checkbox -->
                <span class="shrink-0 flex items-center justify-center">
                  <CheckSquare v-if="selectedTorrentFileIndices.has(item.fileIndex)" :size="15" class="text-purple" />
                  <Square v-else :size="15" class="text-faint" />
                </span>

                <!-- Kind / Ext Badge -->
                <span class="shrink-0 rounded px-1.5 py-0.5 text-[9px] font-mono font-bold uppercase"
                  :class="item.kind === 'audio' ? 'bg-purple/15 text-purple' : 'bg-sky-500/15 text-sky-400'">
                  {{ item.ext }}
                </span>

                <!-- Title / File name -->
                <div class="min-w-0 flex-1 truncate">
                  <span class="text-ink font-medium truncate">{{ item.title }}</span>
                  <span v-if="item.artist && item.artist !== torrentResult.name"
                    class="text-faint text-[10px] ml-1.5 truncate">
                    — {{ item.artist }}
                  </span>
                </div>

                <!-- Companion Thumbnail indicator (if matched with image file) -->
                <span v-if="item.companionThumbnail"
                  class="flex shrink-0 items-center gap-1 rounded bg-emerald-500/15 px-1.5 py-0.5 text-[10px] text-emerald-400 font-medium"
                  :title="`Companion Artwork: ${item.companionThumbnail}`">
                  <ImageIcon :size="11" />
                  <span class="max-w-[100px] truncate">{{ item.companionThumbnail }}</span>
                </span>

                <!-- File Size -->
                <span class="shrink-0 text-right text-[11px] tabular-nums text-faint">
                  {{ item.formattedSize }}
                </span>
              </div>

              <div v-if="!filteredTorrentItems.length" class="py-8 text-center text-[11px] text-faint">
                No files match the filter “{{ activeTorrentExtFilter }}”.
              </div>
            </div>

            <!-- Playlist Name input -->
            <div>
              <label class="block text-[11px] font-semibold text-muted mb-1">Playlist Name</label>
              <input v-model="importName"
                class="w-full rounded-xl border border-line bg-elevated px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60 placeholder:text-faint"
                placeholder="Playlist name" :disabled="importLoading" @keydown.enter="handleImportFile" />
            </div>

            <!-- Actions -->
            <div class="flex justify-end gap-2 pt-1">
              <button type="button" class="playlist-action" :disabled="importLoading" @click="showImportModal = false">
                Cancel
              </button>
              <button type="button"
                class="flex items-center gap-1.5 rounded-xl bg-purple px-4 py-2 text-[12px] font-semibold text-white hover:brightness-110 disabled:opacity-50"
                :disabled="importLoading || selectedTorrentCount === 0" @click="handleImportFile">
                <Loader2 v-if="importLoading" :size="14" class="animate-spin" />
                <span>
                  {{ importLoading ? 'Importing tracks…' : `Import Selected (${selectedTorrentCount}
                  file${selectedTorrentCount === 1 ? '' : 's'})` }}
                </span>
              </button>
            </div>
          </template>

          <!-- Case 2: Standard File Picker (for .m3u, .m3u8 or before selecting torrent) -->
          <template v-else>
            <div>
              <label class="block text-[11px] font-semibold text-muted mb-1">Select .m3u, .m3u8, or .torrent
                file</label>
              <div
                class="flex flex-col items-center justify-center rounded-xl border border-dashed border-line hover:border-purple/60 bg-elevated/40 p-5 cursor-pointer transition-colors"
                @click="fileInput?.click()">
                <template v-if="selectedFile">
                  <FileText :size="24" class="text-purple mb-2" />
                  <div class="text-[12px] font-medium text-ink truncate max-w-full">
                    {{ selectedFile.name }}
                  </div>
                </template>
                <template v-else>
                  <FileText :size="24" class="text-faint mb-2" />
                  <div class="text-[12px] text-muted">
                    Click to browse .m3u or .torrent file from computer
                  </div>
                  <div class="text-[10px] text-faint mt-1">
                    Extracts all audio, video files & companion thumbnails automatically
                  </div>
                </template>
              </div>
            </div>

            <div>
              <label class="block text-[11px] font-semibold text-muted mb-1">Playlist Name (Optional)</label>
              <input v-model="importName"
                class="w-full rounded-xl border border-line bg-elevated px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60 placeholder:text-faint"
                placeholder="e.g. My Bollywood Playlist" :disabled="importLoading" @keydown.enter="handleImportFile" />
            </div>

            <div class="flex justify-end gap-2 pt-2">
              <button type="button" class="playlist-action" :disabled="importLoading" @click="showImportModal = false">
                Cancel
              </button>
              <button type="button"
                class="flex items-center gap-1.5 rounded-xl bg-purple px-4 py-2 text-[12px] font-semibold text-white hover:brightness-110 disabled:opacity-50"
                :disabled="importLoading || !selectedFile" @click="handleImportFile">
                <Loader2 v-if="importLoading" :size="14" class="animate-spin" />
                <span>{{ importLoading ? 'Parsing tracks…' : 'Import File' }}</span>
              </button>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>

  <ContextMenu
    v-if="menu"
    :x="menu.x"
    :y="menu.y"
    :items="menuDefinition.items"
    @close="menu = null"
    @select="onMenuSelect"
  />
</template>

<style scoped>
.playlist-action { display: flex; height: 2.1rem; align-items: center; justify-content: center; gap: 0.35rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.7rem; font-size: 0.7rem; color: var(--text-secondary); }
.playlist-action:hover { color: var(--text-primary); border-color: var(--border-strong); }
.playlist-action:disabled { opacity: 0.4; cursor: not-allowed; }
.action-icon { display: flex; height: 1.6rem; width: 1.6rem; align-items: center; justify-content: center; border-radius: 0.35rem; color: var(--text-muted); }
.action-icon:hover { background: var(--bg-active); color: var(--text-primary); }
</style>
