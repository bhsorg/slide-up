<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Clapperboard, FolderPlus, FilePlus2, LayoutGrid, List, Trash2 } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import type { MediaItem } from "../types/media";

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const viewMode = ref<"grid" | "list">(
  (localStorage.getItem("slideup-video-view") as "grid" | "list" | null) ?? "grid",
);
watch(viewMode, (value) => localStorage.setItem("slideup-video-view", value));

const items = computed(() => {
  const query = library.search.trim().toLowerCase();
  const list = library.videoItems.filter((item) =>
    !query || `${item.title} ${item.artist ?? ""}`.toLowerCase().includes(query),
  );
  return [...list].sort((a, b) => b.addedAt - a.addedAt);
});

function play(list: MediaItem[], index: number) {
  void player.playList(list, index, "Video Library").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}

async function importFiles() {
  try {
    const result = await library.importFiles();
    if (result) toast.success(`Imported ${result.added} video item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Import failed");
  }
}

async function scanFolder() {
  try {
    const result = await library.scanFolder();
    if (result) toast.success(`Scanned folder · ${result.added} new item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Scan failed");
  }
}

async function clearAll() {
  const count = library.videoItems.length;
  if (!count) return;
  if (!window.confirm(`Remove all ${count} videos from the video library? Files on disk will not be deleted.`)) return;
  try {
    const removed = await library.clear("video");
    toast.success(`Removed ${removed} video${removed === 1 ? "" : "s"} from the library.`);
  } catch (error) {
    toast.error(String(error), "Could not clear video library");
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4">
    <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div class="flex items-center gap-3">
        <span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-linear-to-br from-blue to-purple text-white"><Clapperboard :size="21" /></span>
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-ink">Video Library</h1>
          <p class="text-[12px] text-muted">{{ library.videoItems.length }} videos in your collection</p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <div class="flex items-center rounded-xl border border-line bg-panel p-1">
          <button type="button" class="view-toggle" :class="viewMode === 'grid' ? 'bg-purple text-white' : 'text-faint'" title="Grid view" @click="viewMode = 'grid'"><LayoutGrid :size="15" /></button>
          <button type="button" class="view-toggle" :class="viewMode === 'list' ? 'bg-purple text-white' : 'text-faint'" title="List view" @click="viewMode = 'list'"><List :size="15" /></button>
        </div>
        <button type="button" class="library-action" @click="importFiles"><FilePlus2 :size="15" /> Add files</button>
        <button type="button" class="library-action" @click="scanFolder"><FolderPlus :size="15" /> Add folder</button>
        <button type="button" class="library-action danger" title="Clear all videos from library" @click="clearAll"><Trash2 :size="15" /> Clear all</button>
      </div>
    </div>

    <MediaGrid :items="items" :mode="viewMode" context-name="All videos" @play="play" />
  </div>
</template>

<style scoped>
.library-action {
  display: flex;
  height: 2.25rem;
  align-items: center;
  gap: 0.45rem;
  border-radius: 0.7rem;
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0 0.85rem;
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--text-secondary);
  transition: color var(--t-fast), border-color var(--t-fast);
}
.library-action:hover { color: var(--text-primary); border-color: var(--border-strong); }
.library-action.danger:hover { color: #ff5b8d; border-color: #ff5b8d; }
.view-toggle {
  display: flex;
  height: 1.9rem;
  width: 1.9rem;
  align-items: center;
  justify-content: center;
  border-radius: 0.5rem;
  transition: background var(--t-fast), color var(--t-fast);
}
.view-toggle:hover { color: var(--text-primary); background: var(--bg-hover); }
</style>
