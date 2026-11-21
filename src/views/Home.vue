<script setup lang="ts">
import { computed } from "vue";
import { Play, FolderPlus, FilePlus2, Music2, Clapperboard, Heart, Sparkles } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import AppLogo from "../components/AppLogo.vue";
import type { MediaItem } from "../types/media";
import type { ViewId } from "../types/settings";

const emit = defineEmits<{ navigate: [view: ViewId] }>();

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const featured = computed(() => library.recent[0] ?? library.items[0] ?? null);
const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
});

function play(items: MediaItem[], index: number) {
  void player.playList(items, index, "Library").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}

async function addFiles() {
  try {
    const result = await library.importFiles();
    if (result) toast.success(`Imported ${result.added} item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Import failed");
  }
}

async function addFolder() {
  try {
    const result = await library.scanFolder();
    if (result) toast.success(`Scanned folder · ${result.added} new item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Scan failed");
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4 custom-scrollbar">
    <div class="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div>
        <div class="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-purple-soft">
          <Sparkles :size="13" /> Your space
        </div>
        <h1 class="text-xl font-bold tracking-tight text-ink">{{ greeting }} 👋</h1>
        <p class="mt-0.5 text-[12px] text-muted">Everything you love to watch and listen to, in one place.</p>
      </div>
      <div class="flex gap-2">
        <button type="button" class="home-action" @click="addFiles"><FilePlus2 :size="14" /> Add files</button>
        <button type="button" class="home-action" @click="addFolder"><FolderPlus :size="14" /> Add folder</button>
      </div>
    </div>

    <div class="mb-5 grid grid-cols-3 gap-3">
      <button type="button" class="quick-card group" @click="emit('navigate', 'music')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-purple to-blue text-white"><Music2 :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Music</span>
          <span class="block text-[10.5px] text-faint">{{ library.audioItems.length }} tracks</span>
        </span>
      </button>
      <button type="button" class="quick-card group" @click="emit('navigate', 'videos')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-blue to-purple text-white"><Clapperboard :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Videos</span>
          <span class="block text-[10.5px] text-faint">{{ library.videoItems.length }} videos</span>
        </span>
      </button>
      <button type="button" class="quick-card group" @click="emit('navigate', 'favorites')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-[#ff4d8d] to-[#ff9a5c] text-white"><Heart :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Favorites</span>
          <span class="block text-[10.5px] text-faint">{{ library.favorites.length }} liked</span>
        </span>
      </button>
    </div>

    <div v-if="featured" class="su-panel mb-5 flex flex-wrap items-center gap-4 overflow-hidden p-4">
      <div class="relative h-24 w-38 shrink-0 overflow-hidden rounded-lg bg-elevated">
        <img v-if="featured.thumbnailPath" :src="library.thumbnailUrl(featured.thumbnailPath)" :alt="featured.title" class="h-full w-full object-cover" />
        <div v-else class="flex h-full w-full items-center justify-center bg-linear-to-br from-purple/25 to-blue/25 text-white/30">
          <Clapperboard v-if="featured.kind === 'video'" :size="24" />
          <AppLogo v-else class="h-14 w-14 opacity-80" />
        </div>
      </div>
      <div class="min-w-0 flex-1">
        <div class="mb-1 text-[10px] font-semibold uppercase tracking-wider text-purple-soft">Continue where you left off</div>
        <h2 class="truncate text-lg font-bold text-ink">{{ featured.title }}</h2>
        <p class="mt-0.5 truncate text-[11px] text-muted">{{ featured.artist || "Your library" }} · {{ featured.kind === 'video' ? 'Video' : 'Audio' }}</p>
      </div>
      <button type="button" class="flex h-9.5 items-center gap-2 rounded-lg bg-purple px-4 text-[12.5px] font-semibold text-white hover:opacity-90 transition-opacity" @click="player.play(featured)">
        <Play :size="15" fill="currentColor" /> Play now
      </button>
    </div>

    <MediaGrid :items="library.recent.slice(0, 12)" context-name="Recently played" @play="play" />
  </div>
</template>

<style scoped>
.quick-card {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  border-radius: var(--radius-md, 12px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0.85rem;
  transition: border-color var(--t-fast), background var(--t-fast);
}
.quick-card:hover {
  border-color: var(--border-strong);
  background: var(--bg-hover);
}
.home-action {
  display: flex;
  height: 2.1rem;
  align-items: center;
  gap: 0.4rem;
  border-radius: var(--radius-sm, 8px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0 0.75rem;
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--text-secondary);
  transition: color var(--t-fast), border-color var(--t-fast);
}
.home-action:hover {
  color: var(--text-primary);
  border-color: var(--border-strong);
}
</style>