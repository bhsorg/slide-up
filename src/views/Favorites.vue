<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Heart, LayoutGrid, List } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import type { MediaItem } from "../types/media";

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const viewMode = ref<"grid" | "list">(
  (localStorage.getItem("slideup-favorites-view") as "grid" | "list" | null) ?? "grid",
);
watch(viewMode, (value) => localStorage.setItem("slideup-favorites-view", value));

const items = computed(() => library.favorites);

function play(list: MediaItem[], index: number) {
  void player.playList(list, index, "Favorites").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4">
    <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div class="flex items-center gap-3">
        <span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-linear-to-br from-[#ff4d8d] to-[#ff9a5c] text-white"><Heart :size="21" fill="currentColor" /></span>
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-ink">Favorites</h1>
          <p class="text-[12px] text-muted">{{ items.length }} liked item{{ items.length === 1 ? "" : "s" }}</p>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <div class="flex items-center rounded-xl border border-line bg-panel p-1">
          <button
            type="button"
            class="view-toggle"
            :class="viewMode === 'grid' ? 'bg-purple text-white' : 'text-faint'"
            title="Grid view"
            @click="viewMode = 'grid'"
          >
            <LayoutGrid :size="15" />
          </button>
          <button
            type="button"
            class="view-toggle"
            :class="viewMode === 'list' ? 'bg-purple text-white' : 'text-faint'"
            title="List view"
            @click="viewMode = 'list'"
          >
            <List :size="15" />
          </button>
        </div>
      </div>
    </div>

    <MediaGrid :items="items" :mode="viewMode" remove-type="favorites" context-name="Liked media" @play="play" />
  </div>
</template>

<style scoped>
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
