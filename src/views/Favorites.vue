<script setup lang="ts">
import { computed } from "vue";
import { Heart } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import type { MediaItem } from "../types/media";

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const items = computed(() => library.favorites);

function play(list: MediaItem[], index: number) {
  void player.playList(list, index, "Favorites").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4">
    <div class="mb-6 flex items-center gap-3">
      <span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-linear-to-br from-[#ff4d8d] to-[#ff9a5c] text-white"><Heart :size="21" fill="currentColor" /></span>
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-ink">Favorites</h1>
        <p class="text-[12px] text-muted">{{ items.length }} liked item{{ items.length === 1 ? "" : "s" }}</p>
      </div>
    </div>
    <MediaGrid :items="items" context-name="Liked media" @play="play" />
  </div>
</template>
