<script setup lang="ts">
import { computed } from "vue";
import { History } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import type { MediaItem } from "../types/media";

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();

const items = computed(() => library.recent);

function play(list: MediaItem[], index: number) {
  void player.playList(list, index, "Recently played").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4">
    <div class="mb-6 flex items-center gap-3">
      <span class="flex h-11 w-11 items-center justify-center rounded-2xl bg-linear-to-br from-purple to-blue text-white"><History :size="21" /></span>
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-ink">Recently Played</h1>
        <p class="text-[12px] text-muted">Pick up where you left off</p>
      </div>
    </div>
    <MediaGrid :items="items" context-name="Play history" @play="play" />
  </div>
</template>
