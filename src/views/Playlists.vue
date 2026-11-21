<script setup lang="ts">
import { computed, ref } from "vue";
import { ListMusic, Plus, Pencil, Trash2, Download, Upload, Play, X, ArrowUp, ArrowDown, Music2, MoreVertical, ListPlus, Heart, Film, Captions, Info } from "@lucide/vue";
import { usePlaylistStore } from "../stores/playlist";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useMediaProperties } from "../composables/useMediaProperties";
import { useStoryboardTask } from "../composables/useStoryboardTask";
import { api } from "../services/tauri";
import { formatDuration } from "../types/media";
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

const newName = ref("");
const renamingId = ref<number | null>(null);
const renameValue = ref("");
const pickerOpen = ref(false);
const pickerSearch = ref("");

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

async function importPlaylist() {
  try {
    const id = await playlists.importPlaylist();
    if (id != null) toast.success("Playlist imported.");
  } catch (error) {
    toast.error(String(error), "Could not import playlist");
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
    void api.hasSubtitle(item.id).then((value) => {
      subtitleAvailable.value = value;
    }).catch(() => undefined);
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
        <button type="button" class="playlist-action flex-1" @click="importPlaylist"><Upload :size="13" /> Import</button>
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
          <button type="button" class="min-w-0 flex-1 text-left" @click="playlists.select(playlist.id)">
            <div class="truncate text-[12px] font-semibold text-ink">{{ playlist.name }}</div>
            <div class="text-[10px] text-faint">{{ playlist.trackCount }} track{{ playlist.trackCount === 1 ? "" : "s" }}</div>
          </button>
          <button type="button" class="action-icon" title="Rename" @click="renamingId = playlist.id; renameValue = playlist.name"><Pencil :size="12" /></button>
          <button type="button" class="action-icon hover:!text-[#ff5b8d]" title="Delete" @click="remove(playlist.id)"><Trash2 :size="12" /></button>
          <button type="button" class="action-icon" title="More actions" @click="openPlaylistMenu($event, playlist)"><MoreVertical :size="12" /></button>
        </div>
        <div v-if="!playlists.playlists.length" class="mt-8 text-center text-[11px] leading-relaxed text-faint">No playlists yet. Create one above or import an M3U file.</div>
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
            <div class="min-w-0 flex-1 truncate text-[15px] font-semibold text-ink">{{ playlists.active.name }}</div>
            <button type="button" class="playlist-action" @click="pickerOpen = true"><Plus :size="13" /> Add media</button>
          </template>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto">
          <div class="grid grid-cols-[32px_1fr_90px_90px] gap-2 border-b border-line px-5 py-2 text-[10px] font-semibold uppercase tracking-wider text-faint">
            <span>#</span><span>Title</span><span>Album</span><span class="text-right">Duration</span>
          </div>
          <div
            v-for="(entry, index) in playlists.tracks"
            :key="`${entry.media.id}-${index}`"
            class="group grid grid-cols-[32px_1fr_90px_90px] items-center gap-2 border-b border-line/60 px-5 py-2.5 text-[12px] hover:bg-hover/60"
            @contextmenu.prevent="openTrackMenu($event, index)"
          >
            <button type="button" class="flex h-7 w-7 items-center justify-center rounded-md text-faint hover:bg-purple hover:text-white" title="Play" @click="play(index)"><Play :size="12" fill="currentColor" /></button>
            <div class="flex min-w-0 items-center gap-2">
              <Music2 :size="13" class="shrink-0 text-faint" />
              <span class="truncate text-ink">{{ entry.media.title }}</span>
              <span class="ml-auto flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                <button type="button" class="action-icon" title="Move up" @click="move(index, -1)"><ArrowUp :size="12" /></button>
                <button type="button" class="action-icon" title="Move down" @click="move(index, 1)"><ArrowDown :size="12" /></button>
                <button type="button" class="action-icon hover:!text-[#ff5b8d]" title="Remove" @click="removeTrack(entry.media)"><X :size="12" /></button>
                <button type="button" class="action-icon" title="More actions" @click.stop="openTrackMenu($event, index)"><MoreVertical :size="12" /></button>
              </span>
            </div>
            <span class="truncate text-faint">{{ entry.media.album || "—" }}</span>
            <span class="text-right tabular-nums text-faint">{{ formatDuration(entry.media.durationMs) }}</span>
          </div>
          <div v-if="!playlists.tracks.length" class="py-16 text-center text-[12px] text-faint">This playlist is empty. Add media from your library.</div>
        </div>
      </template>
      <div v-else class="flex flex-1 flex-col items-center justify-center text-center">
        <ListMusic :size="38" class="mb-3 text-faint" />
        <p class="text-sm font-medium text-muted">No playlist selected</p>
        <p class="mt-1 text-xs text-faint">Create or select a playlist to see its tracks.</p>
      </div>
    </div>

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
