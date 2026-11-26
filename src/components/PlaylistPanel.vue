<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Plus, MoreVertical, Play, Heart, ListPlus, ListMusic, Trash2, X, Film, Captions, Info, Tv } from "@lucide/vue";
import { usePlayerStore } from "../stores/player";
import { useLibraryStore } from "../stores/library";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useMediaProperties } from "../composables/useMediaProperties";
import { useStoryboardTask } from "../composables/useStoryboardTask";
import { useSubtitleAvailability } from "../composables/useSubtitleAvailability";
import { formatDuration, type MediaItem } from "../types/media";
import ContextMenu from "./ContextMenu.vue";
import AppLogo from "./AppLogo.vue";
import type { ContextMenuItem } from "../types/contextMenu";

defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const player = usePlayerStore();
const library = useLibraryStore();
const toast = useToast();
const picker = usePlaylistPicker();
const properties = useMediaProperties();
const storyboard = useStoryboardTask();
const subtitles = useSubtitleAvailability();

const list = computed(() => {
  if (player.queue.length > 0) {
    return player.queue;
  }
  return library.items.slice(0, 8);
});

const activeIndex = computed(() => player.currentIndex ?? 0);

watch(list, (items) => {
  void subtitles.checkMany(items);
}, { immediate: true });

function hasCaption(item: MediaItem): boolean {
  return item.kind === "video" && subtitles.has(item.id);
}

async function playAt(index: number) {
  if (player.queue.length > 0) {
    await player.queuePlayAt(index);
  } else {
    await player.playList(list.value, index, "Playlist");
  }
}

async function addMediaPrompt() {
  try {
    const result = await library.importFiles();
    if (result) toast.success(`Added ${result.added} items.`);
  } catch (error) {
    toast.error(String(error), "Import failed");
  }
}

const menu = ref<{ x: number; y: number; index: number | null } | null>(null);
const subtitleAvailable = ref(false);

function openItemMenu(event: MouseEvent, index: number) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = { x: event.clientX, y: event.clientY, index };
  subtitleAvailable.value = false;
  const item = list.value[index];
  if (item?.kind === "video") {
    void storyboard.check(item.id);
    subtitleAvailable.value = subtitles.get(item.id) ?? false;
    void subtitles.check(item.id).then((value) => {
      subtitleAvailable.value = value;
    });
  }
}

function openHeaderMenu(event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = { x: event.clientX, y: event.clientY, index: null };
}

const menuDefinition = computed<{ items: ContextMenuItem[]; actions: string[] }>(() => {
  const current = menu.value;
  if (!current) return { items: [], actions: [] };
  if (current.index === null) {
    return {
      items: [
        { label: "Clear queue", icon: Trash2, disabled: player.queue.length === 0 },
        { label: "Close panel", icon: X },
      ],
      actions: ["clear", "close"],
    };
  }
  const item = list.value[current.index];
  if (!item) return { items: [], actions: [] };
  const inQueue = player.queue.length > 0;
  const items: ContextMenuItem[] = [
    { label: "Play now", icon: Play },
    { label: item.favorite ? "Remove from favorites" : "Add to favorites", icon: Heart },
    { label: "Play next", icon: ListPlus },
    inQueue
      ? { label: "Remove from queue", icon: X, danger: true }
      : { label: "Add to queue", icon: ListMusic },
    { label: "Add to playlist…", icon: Plus },
    { label: "Properties…", icon: Info },
  ];
  const actions = ["play", "favorite", "next", "queue", "playlist", "properties"];
  if (item.kind === "video") {
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
  return { items, actions };
});

async function onMenuSelect(index: number) {
  const current = menu.value;
  if (!current) return;
  const action = menuDefinition.value.actions[index];
  if (current.index === null) {
    if (action === "clear") {
      try {
        await player.queueClear();
        toast.info("Queue cleared.");
      } catch (error) {
        toast.error(String(error), "Could not clear queue");
      }
    } else {
      emit("close");
    }
    return;
  }
  const item = list.value[current.index];
  if (!item) return;
  const inQueue = player.queue.length > 0;
  switch (action) {
    case "play":
      await playAt(current.index);
      break;
    case "favorite":
      try {
        await library.toggleFavorite(item);
      } catch (error) {
        toast.error(String(error), "Could not update favorite");
      }
      break;
    case "next":
      try {
        await player.queueNext(item);
        toast.success(`Playing next: “${item.title}”.`);
      } catch (error) {
        toast.error(String(error), "Could not queue item");
      }
      break;
    case "queue":
      try {
        if (inQueue) {
          await player.queueRemove(current.index);
          toast.info(`Removed “${item.title}” from the queue.`);
        } else {
          await player.queueAdd(item);
          toast.success(`Queued “${item.title}”.`);
        }
      } catch (error) {
        toast.error(String(error), "Could not update queue");
      }
      break;
    case "playlist":
      picker.askAddToPlaylist(item);
      break;
    case "properties":
      properties.showProperties(item);
      break;
    case "storyboard":
      try {
        await storyboard.generate(item.id);
        player.reloadStoryboard(item.id);
        toast.success(`Storyboard ready for “${item.title}”.`);
      } catch (error) {
        toast.error(String(error), "Could not generate storyboard");
      }
      break;
    case "subtitle":
      await player.openSubtitle();
      break;
  }
}
</script>

<template>
  <aside
    class="flex h-full w-[var(--playlist-width,290px)] shrink-0 flex-col overflow-hidden border-l border-line bg-secondary transition-[margin,opacity] duration-[var(--t-base)]"
    :class="open ? 'mr-0 opacity-100' : 'pointer-events-none -mr-[var(--playlist-width,290px)] opacity-0'"
  >
    <!-- Header (matching slideup-ui.png: "Playlist (8)  +  ⋮") -->
    <div class="flex h-[var(--topbar-height,52px)] shrink-0 items-center justify-between border-b border-line px-4">
      <h2 class="text-[13.5px] font-bold tracking-tight text-ink">
        Playlist ({{ list.length }})
      </h2>
      <div class="flex items-center gap-0.5">
        <button
          type="button"
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted hover:bg-hover hover:text-ink transition-colors"
          title="Add files to playlist"
          @click="addMediaPrompt"
        >
          <Plus :size="16" />
        </button>
        <button
          type="button"
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted hover:bg-hover hover:text-ink transition-colors"
          title="Playlist options"
          @click="openHeaderMenu"
        >
          <MoreVertical :size="15" />
        </button>
      </div>
    </div>

    <!-- Playlist Track List (clean, compact) -->
    <div class="min-h-0 flex-1 overflow-y-auto px-2.5 py-2 space-y-1 custom-scrollbar">
      <div
        v-for="(item, index) in list"
        :key="`${item.id}-${index}`"
        class="group flex w-full items-center gap-2 rounded-lg p-1.5 text-left cursor-pointer transition-colors"
        :class="activeIndex === index
          ? 'bg-purple/15 border border-purple/40 text-purple'
          : 'hover:bg-hover border border-transparent text-ink'"
        @click="playAt(index)"
        @contextmenu.prevent="openItemMenu($event, index)"
      >
        <!-- Animated Equalizer bars for active track -->
        <div v-if="activeIndex === index" class="flex items-end gap-[2px] h-3 px-0.5 shrink-0" aria-hidden="true">
          <span class="w-[2px] h-2.5 bg-purple rounded-full" :class="player.isPlaying ? 'animate-pulse' : 'opacity-60'" />
          <span class="w-[2px] h-1.5 bg-purple rounded-full" :class="player.isPlaying ? 'animate-pulse' : 'opacity-60'" style="animation-delay: 150ms" />
          <span class="w-[2px] h-3 bg-purple rounded-full" :class="player.isPlaying ? 'animate-pulse' : 'opacity-60'" style="animation-delay: 300ms" />
        </div>

        <!-- Track Thumbnail -->
        <div class="relative h-9 w-9 shrink-0 overflow-hidden rounded-md bg-panel border border-line">
          <img
            v-if="library.coverUrl(item)"
            :src="library.coverUrl(item)"
            :alt="item.title"
            loading="lazy"
            class="h-full w-full object-cover z-1"
            @error="($event.target as HTMLImageElement).style.display = 'none'"
          />
          <div class="flex h-full w-full items-center justify-center text-faint bg-panel">
            <Tv v-if="item.id < -1000 || item.artist === 'Live TV'" :size="16" class="text-purple-400 opacity-90" />
            <AppLogo v-else class="h-5 w-5 opacity-80" />
          </div>
        </div>

        <!-- Track Title & Subtitle -->
        <div class="min-w-0 flex-1">
          <div class="flex min-w-0 items-center gap-1.5">
            <span
              class="truncate text-[12px] font-semibold"
              :class="activeIndex === index ? 'text-purple' : 'text-ink'"
            >{{ item.title }}</span>
            <span
              v-if="hasCaption(item)"
              class="shrink-0 rounded bg-purple/20 px-1 text-[9px] font-bold leading-4 text-purple"
              title="Subtitles available"
            >CC</span>
          </div>
          <div class="truncate text-[10px] text-muted">
            {{ item.artist || (item.kind === 'video' ? 'Video' : 'Unknown Artist') }}
          </div>
        </div>

        <!-- Track Duration & More Dots -->
        <div class="flex items-center gap-1 shrink-0">
          <span
            v-if="item.codec === 'Live HLS' || item.artist === 'Live TV'"
            class="live-dot-badge scale-90"
          >
            <span class="live-dot"></span>
            LIVE
          </span>
          <span v-else-if="item.durationMs > 0" class="text-[10.5px] tabular-nums text-muted">
            {{ formatDuration(item.durationMs) }}
          </span>
          <span v-else class="text-[10.5px] tabular-nums text-muted">
            {{ item.path.startsWith('http') ? 'Stream' : '--:--' }}
          </span>
          <button
            type="button"
            class="text-[#929bb8] hover:text-white"
            title="Item options"
            @click.stop="openItemMenu($event, index)"
          >
            <MoreVertical :size="14" />
          </button>
        </div>
      </div>
    </div>
  </aside>

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
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(146, 155, 184, 0.2);
  border-radius: 9999px;
}
</style>
