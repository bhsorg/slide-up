<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Play, Heart, ListPlus, Clapperboard, Music2, Plus, ListMusic, MoreVertical, Trash2, Film, Captions, Info, Check, X, User } from "@lucide/vue";
import type { MediaItem } from "../types/media";
import { formatDuration } from "../types/media";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useMediaProperties } from "../composables/useMediaProperties";
import { useArtistBio } from "../composables/useArtistBio";
import { useStoryboardTask } from "../composables/useStoryboardTask";
import { useSubtitleAvailability } from "../composables/useSubtitleAvailability";
import { confirmAction } from "../utils/dialog";
import ContextMenu from "./ContextMenu.vue";
import AppLogo from "./AppLogo.vue";
import type { ContextMenuItem } from "../types/contextMenu";

const props = withDefaults(defineProps<{
  items: MediaItem[];
  contextName?: string;
  mode?: "grid" | "list";
  selectable?: boolean;
  removeType?: "library" | "recent" | "favorites" | "none";
}>(), {
  mode: "grid",
  selectable: true,
  removeType: "library",
});
const emit = defineEmits<{ play: [items: MediaItem[], index: number] }>();

const library = useLibraryStore();
const player = usePlayerStore();
const toast = useToast();
const picker = usePlaylistPicker();
const properties = useMediaProperties();
const { showArtist } = useArtistBio();
const storyboard = useStoryboardTask();
const subtitles = useSubtitleAvailability();

const title = computed(() => props.contextName ?? "Media");

// Resolve subtitle availability for every visible video so cards can show a CC
// badge. Results are cached by the composable across views and re-renders.
watch(() => props.items, (items) => {
  void subtitles.checkMany(items);
  for (const item of items) void library.requestVideoCover(item);
}, { immediate: true });

function hasCaption(item: MediaItem): boolean {
  return item.kind === "video" && subtitles.has(item.id);
}

function playAt(index: number) {
  emit("play", props.items, index);
}

async function favorite(item: MediaItem) {
  try {
    await library.toggleFavorite(item);
  } catch (error) {
    toast.error(String(error), "Could not update favorite");
  }
}

async function queue(item: MediaItem) {
  try {
    await player.queueAdd(item);
    toast.success(`Queued “${item.title}”.`);
  } catch (error) {
    toast.error(String(error), "Could not queue item");
  }
}

function addToPlaylist(item: MediaItem) {
  picker.askAddToPlaylist(item);
}

// Multi-select. Clicking a checkbox toggles an item without playing it; the
// bulk bar appears as soon as at least one item in the current list is picked.
const selectedIds = ref<Set<number>>(new Set());
const selectedItems = computed(() => props.items.filter((item) => selectedIds.value.has(item.id)));

function toggleSelected(id: number) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  selectedIds.value = next;
}

function clearSelection() {
  selectedIds.value = new Set();
}

const allSelected = computed(() => props.items.length > 0 && props.items.every((item) => selectedIds.value.has(item.id)));
const someSelected = computed(() => selectedItems.value.length > 0 && !allSelected.value);

function toggleSelectAll() {
  if (allSelected.value) {
    clearSelection();
  } else {
    selectedIds.value = new Set(props.items.map((item) => item.id));
  }
}

function addSelectedToPlaylist() {
  if (!selectedItems.value.length) return;
  picker.askAddToPlaylist(selectedItems.value);
  clearSelection();
}

async function queueSelected() {
  const items = selectedItems.value;
  if (!items.length) return;
  try {
    for (const item of items) {
      await player.queueAdd(item);
    }
    toast.success(`Queued ${items.length} item${items.length === 1 ? "" : "s"}.`);
    clearSelection();
  } catch (error) {
    toast.error(String(error), "Could not queue items");
  }
}

async function removeSelected() {
  const items = selectedItems.value;
  if (!items.length) return;
  const ids = items.map((item) => item.id);

  if (props.removeType === "recent") {
    const confirmed = await confirmAction(
      `Remove ${items.length} item${items.length === 1 ? "" : "s"} from recent playback history?`,
      { title: "Remove from History", kind: "info" }
    );
    if (!confirmed) return;
    try {
      await library.removeFromRecent(ids);
      toast.success(`Removed ${items.length} item${items.length === 1 ? "" : "s"} from recent history.`);
      clearSelection();
    } catch (error) {
      toast.error(String(error), "Could not remove items from recent history");
    }
  } else if (props.removeType === "favorites") {
    const confirmed = await confirmAction(
      `Remove ${items.length} item${items.length === 1 ? "" : "s"} from favorites?`,
      { title: "Remove from Favorites", kind: "info" }
    );
    if (!confirmed) return;
    try {
      await library.removeFavorites(ids);
      toast.success(`Removed ${items.length} item${items.length === 1 ? "" : "s"} from favorites.`);
      clearSelection();
    } catch (error) {
      toast.error(String(error), "Could not remove items from favorites");
    }
  } else if (props.removeType === "library") {
    const confirmed = await confirmAction(
      `Remove ${items.length} item${items.length === 1 ? "" : "s"} from the library?\n\nFiles on disk will not be deleted.`,
      { title: "Remove from Library", kind: "warning", okLabel: "Remove" }
    );
    if (!confirmed) return;
    try {
      await library.removeMany(ids);
      toast.success(`Removed ${items.length} item${items.length === 1 ? "" : "s"} from the library.`);
      clearSelection();
    } catch (error) {
      toast.error(String(error), "Could not remove items from library");
    }
  }
}

const menu = ref<{ x: number; y: number; index: number } | null>(null);
const subtitleAvailable = ref(false);

function openMenu(event: MouseEvent, index: number) {
  event.preventDefault();
  event.stopPropagation();
  menu.value = { x: event.clientX, y: event.clientY, index };
  subtitleAvailable.value = false;
  const item = props.items[index];
  if (item?.kind === "video") {
    void storyboard.check(item.id);
    subtitleAvailable.value = subtitles.get(item.id) ?? false;
    void subtitles.check(item.id).then((value) => {
      subtitleAvailable.value = value;
    });
  }
}

const menuDefinition = computed<{ items: ContextMenuItem[]; actions: string[] }>(() => {
  const item = menu.value ? props.items[menu.value.index] : null;
  if (!item) return { items: [], actions: [] };
  const items: ContextMenuItem[] = [
    { label: "Play now", icon: Play },
    { label: item.favorite ? "Remove from favorites" : "Add to favorites", icon: Heart },
    { label: "Play next", icon: ListPlus },
    { label: "Add to queue", icon: ListMusic },
    { label: "Add to playlist…", icon: Plus },
    { label: "Properties…", icon: Info },
  ];
  const actions = ["play", "favorite", "next", "queue", "playlist", "properties"];
  if (item.artist && !/unknown|various/i.test(item.artist)) {
    items.push({ label: `About ${item.artist}…`, icon: User });
    actions.push("artist");
  }
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
  if (props.removeType !== "none") {
    items.push({ label: "", separator: true });
    actions.push("separator");
    if (props.removeType === "recent") {
      items.push({ label: "Remove from history", icon: Trash2, danger: true });
      actions.push("remove-recent");
    } else if (props.removeType === "favorites") {
      items.push({ label: "Remove from favorites", icon: Trash2, danger: true });
      actions.push("remove-favorite");
    } else {
      items.push({ label: "Remove from library", icon: Trash2, danger: true });
      actions.push("remove");
    }
  }
  return { items, actions };
});

async function onMenuSelect(index: number) {
  const current = menu.value;
  if (!current) return;
  const item = props.items[current.index];
  if (!item) return;
  const action = menuDefinition.value.actions[index];
  switch (action) {
    case "play":
      playAt(current.index);
      break;
    case "favorite":
      await favorite(item);
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
      await queue(item);
      break;
    case "playlist":
      await addToPlaylist(item);
      break;
    case "properties":
      properties.showProperties(item);
      break;
    case "artist":
      if (item.artist) void showArtist(item.artist);
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
    case "remove-recent":
      try {
        await library.removeFromRecent([item.id]);
        toast.info(`Removed “${item.title}” from history.`);
      } catch (error) {
        toast.error(String(error), "Could not remove item");
      }
      break;
    case "remove-favorite":
      try {
        await library.toggleFavorite(item);
        toast.info(`Removed “${item.title}” from favorites.`);
      } catch (error) {
        toast.error(String(error), "Could not remove item");
      }
      break;
    case "remove":
      {
        const confirmed = await confirmAction(
          `Remove “${item.title}” from the library?\n\nFiles on disk will not be deleted.`,
          { title: "Remove from Library", kind: "warning", okLabel: "Remove" }
        );
        if (!confirmed) return;
        try {
          await library.remove(item);
          toast.info(`Removed “${item.title}” from the library.`);
        } catch (error) {
          toast.error(String(error), "Could not remove item");
        }
      }
      break;
  }
}
</script>

<template>
  <section v-if="items.length" class="space-y-5">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold text-ink">{{ title }}</h2>
      <div class="flex items-center gap-3">
        <label v-if="selectable" class="flex cursor-pointer select-none items-center gap-1.5 text-xs text-muted hover:text-ink" title="Select all items">
          <input
            type="checkbox"
            class="h-3.5 w-3.5 cursor-pointer accent-purple"
            :checked="allSelected"
            :indeterminate="someSelected"
            @change="toggleSelectAll"
          />
          Select all
        </label>
        <span class="text-xs text-faint">{{ items.length }} item{{ items.length === 1 ? "" : "s" }}</span>
      </div>
    </div>

    <div
      v-if="selectable && selectedItems.length"
      class="sticky top-2 z-30 flex items-center gap-2 rounded-xl border border-line-strong bg-elevated/95 px-3 py-2 shadow-[var(--shadow-panel)] backdrop-blur"
    >
      <span class="mr-1 text-xs font-medium text-ink">{{ selectedItems.length }} selected</span>
      <button
        type="button"
        class="flex h-7 items-center gap-1.5 rounded-lg bg-purple/15 px-2.5 text-[11px] font-medium text-purple hover:bg-purple/25"
        title="Add selected items to a playlist"
        @click="addSelectedToPlaylist"
      >
        <Plus :size="13" /> Add to playlist
      </button>
      <button
        type="button"
        class="flex h-7 items-center gap-1.5 rounded-lg bg-active px-2.5 text-[11px] font-medium text-ink hover:bg-hover"
        title="Add selected items to the queue"
        @click="queueSelected"
      >
        <ListPlus :size="13" /> Add to queue
      </button>
      <button
        v-if="removeType !== 'none'"
        type="button"
        class="flex h-7 items-center gap-1.5 rounded-lg bg-[#ff5b8d]/15 px-2.5 text-[11px] font-medium text-[#ff5b8d] hover:bg-[#ff5b8d]/25 transition-colors"
        :title="removeType === 'recent' ? 'Remove from history' : removeType === 'favorites' ? 'Remove from favorites' : 'Remove from library'"
        @click="removeSelected"
      >
        <Trash2 :size="13" />
        <span>{{ removeType === 'recent' ? 'Remove from history' : removeType === 'favorites' ? 'Remove from favorites' : 'Remove from library' }}</span>
      </button>
      <button
        type="button"
        class="ml-auto flex h-7 items-center gap-1 rounded-lg px-2 text-[11px] text-muted hover:bg-active hover:text-ink"
        title="Clear selection"
        @click="clearSelection"
      >
        <X :size="13" /> Clear
      </button>
    </div>

    <template v-if="mode !== 'list'">
    <div class="grid grid-cols-[repeat(auto-fill,minmax(168px,1fr))] gap-5">
      <article
        v-for="(item, index) in items"
        :key="item.id"
        class="group relative overflow-hidden rounded-2xl border border-line bg-panel transition-all duration-[var(--t-base)] hover:-translate-y-1 hover:border-line-strong hover:shadow-[var(--shadow-panel)]"
        @contextmenu.prevent="openMenu($event, index)"
      >
        <button
          v-if="selectable"
          type="button"
          class="absolute left-2 top-2 z-10 flex h-6 w-6 items-center justify-center rounded-full border transition-all"
          :class="selectedIds.has(item.id)
            ? 'border-purple bg-purple text-white opacity-100'
            : 'border-white/60 bg-black/45 text-transparent opacity-0 group-hover:opacity-100 hover:border-white'"
          :title="selectedIds.has(item.id) ? 'Deselect' : 'Select'"
          @click.stop="toggleSelected(item.id)"
        >
          <Check :size="14" :stroke-width="3" />
        </button>
        <button type="button" class="block w-full text-left" @click="playAt(index)">
          <div class="relative aspect-square overflow-hidden bg-elevated">
            <img
              v-if="library.coverUrl(item)"
              :src="library.coverUrl(item)"
              :alt="item.title"
              class="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
              loading="lazy"
            />
            <div v-else class="flex h-full w-full items-center justify-center bg-linear-to-br from-purple/30 via-panel to-blue/25">
              <Clapperboard v-if="item.kind === 'video'" :size="44" class="text-white/25" :stroke-width="1.2" />
              <AppLogo v-else class="h-16 w-16 opacity-80" />
            </div>
            <div class="absolute inset-0 bg-black/25 opacity-0 transition-opacity group-hover:opacity-100" />
            <span
              v-if="hasCaption(item)"
              class="absolute bottom-2 left-2 flex items-center gap-0.5 rounded-md bg-black/70 px-1.5 py-0.5 text-[10px] font-bold tracking-wide text-white/90 backdrop-blur"
              title="Subtitles available"
            >
              <Captions :size="11" :stroke-width="2.2" /> CC
            </span>
            <span class="absolute bottom-2 right-2 rounded-md bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white/90">
              {{ formatDuration(item.durationMs) }}
            </span>
            <span class="absolute left-1/2 top-1/2 flex h-10 w-10 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-purple text-white opacity-0 shadow-[var(--glow-purple)] transition-all group-hover:opacity-100">
              <Play :size="18" fill="currentColor" />
            </span>
          </div>
          <div class="px-3 py-3">
            <div class="truncate text-[13px] font-semibold text-ink">{{ item.title }}</div>
            <div class="mt-0.5 truncate text-[11px] text-faint">{{ item.artist || (item.kind === 'video' ? "Video" : "Unknown artist") }}</div>
          </div>
        </button>
        <div class="absolute right-2 top-2 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            type="button"
            class="flex h-7 w-7 items-center justify-center rounded-lg bg-black/55 text-white backdrop-blur hover:bg-purple"
            :title="item.favorite ? 'Remove from favorites' : 'Add to favorites'"
            @click.stop="favorite(item)"
          >
            <Heart :size="14" :fill="item.favorite ? 'currentColor' : 'none'" :class="item.favorite ? 'text-[#ff5b8d]' : ''" />
          </button>
          <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg bg-black/55 text-white backdrop-blur hover:bg-purple" title="Add to queue" @click.stop="queue(item)">
            <ListPlus :size="14" />
          </button>
          <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg bg-black/55 text-white backdrop-blur hover:bg-purple" title="Add to playlist…" @click.stop="addToPlaylist(item)">
            <Plus :size="14" />
          </button>
          <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg bg-black/55 text-white backdrop-blur hover:bg-purple" title="More actions" @click.stop="openMenu($event, index)">
            <MoreVertical :size="14" />
          </button>
        </div>
      </article>
    </div>
    </template>

    <template v-else>
      <div class="su-panel overflow-hidden">
        <div
          class="hidden gap-3 border-b border-line px-3 py-2 text-[10px] font-semibold uppercase tracking-wide text-faint sm:grid"
          :class="selectable ? 'grid-cols-[18px_40px_minmax(0,1.6fr)_minmax(0,1fr)_80px_112px]' : 'grid-cols-[40px_minmax(0,1.6fr)_minmax(0,1fr)_80px_112px]'"
        >
          <span v-if="selectable"></span>
          <span></span>
          <span>Title</span>
          <span>Artist / Album</span>
          <span>Length</span>
          <span class="text-right">Actions</span>
        </div>
        <div
          v-for="(item, index) in items"
          :key="item.id"
          class="group relative items-center gap-3 border-b border-line/60 px-3 py-2 transition-colors last:border-b-0 hover:bg-hover/60"
          :class="selectable
            ? 'grid grid-cols-[18px_40px_minmax(0,1fr)_auto] sm:grid-cols-[18px_40px_minmax(0,1.6fr)_minmax(0,1fr)_80px_112px]'
            : 'grid grid-cols-[40px_minmax(0,1fr)_auto] sm:grid-cols-[40px_minmax(0,1.6fr)_minmax(0,1fr)_80px_112px]'"
          @contextmenu.prevent="openMenu($event, index)"
        >
          <button
            v-if="selectable"
            type="button"
            class="flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full border transition-all"
            :class="selectedIds.has(item.id)
              ? 'border-purple bg-purple text-white'
              : 'border-line-strong bg-elevated text-transparent hover:border-purple'"
            :title="selectedIds.has(item.id) ? 'Deselect' : 'Select'"
            @click.stop="toggleSelected(item.id)"
          >
            <Check :size="11" :stroke-width="3" />
          </button>
          <button type="button" class="relative h-10 w-10 overflow-hidden rounded-lg bg-elevated" :title="`Play ${item.title}`" @click="playAt(index)">
            <img
              v-if="library.coverUrl(item)"
              :src="library.coverUrl(item)"
              :alt="item.title"
              class="h-full w-full object-cover"
              loading="lazy"
            />
            <span v-else class="flex h-full w-full items-center justify-center text-white/30">
              <Clapperboard v-if="item.kind === 'video'" :size="18" />
              <AppLogo v-else class="h-7 w-7 opacity-80" />
            </span>
            <span class="absolute inset-0 flex items-center justify-center bg-black/55 opacity-0 group-hover:opacity-100">
              <Play :size="14" fill="currentColor" class="text-white" />
            </span>
          </button>

          <button type="button" class="min-w-0 text-left" @click="playAt(index)">
            <div class="flex min-w-0 items-center gap-1.5">
              <span class="truncate text-[12px] font-medium text-ink">{{ item.title }}</span>
              <span
                v-if="hasCaption(item)"
                class="shrink-0 rounded bg-purple/20 px-1 text-[9px] font-bold leading-4 text-purple"
                title="Subtitles available"
              >CC</span>
            </div>
            <div class="truncate text-[10px] text-faint sm:hidden">{{ item.artist || (item.kind === 'video' ? 'Video' : 'Unknown artist') }} · {{ formatDuration(item.durationMs) }}</div>
          </button>

          <div class="hidden min-w-0 truncate text-[11px] text-muted sm:block">
            {{ item.artist || (item.kind === 'video' ? 'Video' : 'Unknown artist') }}
            <span v-if="item.album" class="text-faint"> · {{ item.album }}</span>
          </div>
          <div class="hidden text-[11px] tabular-nums text-faint sm:block">{{ formatDuration(item.durationMs) }}</div>

          <div class="flex justify-end gap-1">
            <button
              type="button"
              class="flex h-7 w-7 items-center justify-center rounded-lg text-faint hover:bg-active hover:text-ink"
              :title="item.favorite ? 'Remove from favorites' : 'Add to favorites'"
              @click="favorite(item)"
            >
              <Heart :size="14" :fill="item.favorite ? 'currentColor' : 'none'" :class="item.favorite ? 'text-[#ff5b8d]' : ''" />
            </button>
            <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg text-faint hover:bg-active hover:text-ink" title="Add to queue" @click="queue(item)">
              <ListPlus :size="14" />
            </button>
            <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg text-faint hover:bg-active hover:text-ink" title="Add to playlist…" @click="addToPlaylist(item)">
              <Plus :size="14" />
            </button>
            <button type="button" class="flex h-7 w-7 items-center justify-center rounded-lg text-faint hover:bg-active hover:text-ink" title="More actions" @click.stop="openMenu($event, index)">
              <MoreVertical :size="14" />
            </button>
          </div>
        </div>
      </div>
    </template>
  </section>

  <div v-else class="flex min-h-[280px] flex-col items-center justify-center rounded-2xl border border-dashed border-line bg-panel/40 text-center">
    <div class="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-elevated text-faint"><Music2 :size="22" /></div>
    <p class="text-sm font-medium text-muted">Nothing here yet</p>
    <p class="mt-1 max-w-xs text-xs text-faint">Add media from the Import button in the top bar or use Ctrl+O / Ctrl+Shift+O.</p>
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
