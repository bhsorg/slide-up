<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { History, LayoutGrid, List, Trash2 } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import type { MediaItem } from "../types/media";

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const viewMode = ref<"grid" | "list">(
  (localStorage.getItem("slideup-recent-view") as "grid" | "list" | null) ?? "grid",
);
watch(viewMode, (value) => localStorage.setItem("slideup-recent-view", value));

const items = computed(() => library.recent);

function play(list: MediaItem[], index: number) {
  void player.playList(list, index, "Recently played").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}

async function clearAllRecent() {
  if (!items.value.length) return;
  if (!window.confirm("Clear all playback history? Media files will not be deleted.")) return;
  try {
    await library.clearRecent();
    toast.success("Cleared playback history.");
  } catch (error) {
    toast.error(String(error), "Could not clear history");
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4">
    <div class="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div class="flex items-center gap-3">
        <span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-linear-to-br from-purple to-blue text-white"><History :size="21" /></span>
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-ink">Recently Played</h1>
          <p class="text-[12px] text-muted">{{ items.length }} recent item{{ items.length === 1 ? "" : "s" }}</p>
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

        <button
          type="button"
          class="library-action danger"
          :disabled="!items.length"
          title="Clear all playback history"
          @click="clearAllRecent"
        >
          <Trash2 :size="15" /> Clear recent
        </button>
      </div>
    </div>

    <MediaGrid :items="items" :mode="viewMode" remove-type="recent" context-name="Play history" @play="play" />
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
.library-action:disabled { opacity: 0.4; pointer-events: none; }
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
