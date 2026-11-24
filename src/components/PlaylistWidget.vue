<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { emit as emitAppEvent } from "@tauri-apps/api/event";
import { Film, ListMusic, Music2, Search, X } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { events } from "../services/tauri";
import { formatDuration } from "../types/media";

const player = usePlayerStore();
const library = useLibraryStore();
const playlistWindow = getCurrentWindow();
const ready = ref(false);
const errorMessage = ref("");
const searchQuery = ref("");
const filteredQueue = computed(() => {
  const query = searchQuery.value.trim().toLocaleLowerCase();
  return player.queue
    .map((item, index) => ({
      item,
      index,
      thumbnailUrl: library.thumbnailUrl(
        (item.id > 0 ? library.byId(item.id)?.thumbnailPath : null) ?? item.thumbnailPath,
      ),
    }))
    .filter(({ item }) =>
      !query ||
      [item.title, item.artist, item.album]
        .filter((value): value is string => !!value)
        .some((value) => value.toLocaleLowerCase().includes(query)),
    );
});

onMounted(async () => {
  document.getElementById("boot-splash")?.remove();
  void getCurrentWebview().setBackgroundColor([0, 0, 0, 0]).catch((error: unknown) => {
    errorMessage.value = String(error);
  });
  try {
    await player.initialize();
    ready.value = true;
  } catch (error) {
    errorMessage.value = String(error);
  }
  try {
    await library.initialize();
  } catch (error) {
    errorMessage.value = errorMessage.value || String(error);
  }
});

onBeforeUnmount(() => {
  player.dispose();
  library.dispose();
});

async function playAt(index: number) {
  try {
    await player.queuePlayAt(index);
  } catch (error) {
    errorMessage.value = String(error);
  }
}

async function closeWidget() {
  try {
    await emitAppEvent(events.playlistWidgetVisibilityChanged, false);
    await playlistWindow.hide();
  } catch (error) {
    errorMessage.value = String(error);
  }
}

function onDragStart(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, textarea, a, label, [data-no-drag]")) return;
  event.preventDefault();
  void playlistWindow.startDragging().catch((error: unknown) => {
    errorMessage.value = String(error);
  });
}
</script>

<template>
  <div class="playlist-widget" :class="{ 'is-ready': ready }" @mousedown="onDragStart">
    <div class="playlist-panel">
      <header class="widget-header">
        <div class="widget-icon">
          <ListMusic :size="17" />
        </div>
        <div class="widget-title">
          <div class="title">Playlist</div>
          <div class="subtitle">{{ player.queue.length }} track{{ player.queue.length === 1 ? "" : "s" }}</div>
        </div>
        <button class="icon-button" type="button" title="Close playlist" @click="closeWidget">
          <X :size="16" />
        </button>
      </header>

      <label class="queue-search">
        <Search :size="15" aria-hidden="true" />
        <input
          v-model="searchQuery"
          type="search"
          placeholder="Search playlist…"
          aria-label="Search playlist"
        />
      </label>

      <p v-if="errorMessage || player.state.error" class="widget-error" role="alert">
        {{ errorMessage || player.state.error }}
      </p>

      <div v-if="!player.queue.length" class="queue-empty">The queue is empty.</div>
      <div v-else-if="!filteredQueue.length" class="queue-empty">No matching tracks.</div>
      <div v-else class="queue" data-no-drag>
        <button
          v-for="{ item, index } in filteredQueue"
          :key="`${item.id}-${index}`"
          class="queue-row"
          :class="{ current: index === player.currentIndex }"
          type="button"
          :title="item.title"
          @click="playAt(index)"
        >
          <span class="queue-index">{{ index === player.currentIndex ? "▶" : index + 1 }}</span>
          <img
            v-if="thumbnailUrl"
            class="queue-thumbnail"
            :src="thumbnailUrl"
            alt=""
            draggable="false"
          />
          <span v-else class="queue-thumbnail queue-thumbnail-placeholder">
            <Film v-if="item.kind === 'video'" :size="17" />
            <Music2 v-else :size="17" />
          </span>
          <span class="queue-meta">
            <span class="queue-title">{{ item.title }}</span>
            <span class="queue-artist">{{ item.artist || item.album || item.kind }}</span>
          </span>
          <span class="queue-time">{{ formatDuration(item.durationMs) }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
  background: transparent;
}

.playlist-widget {
  box-sizing: border-box;
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 16px;
  clip-path: inset(0 round 16px);
  background: transparent;
  color: var(--text-primary);
  opacity: 0;
  user-select: none;
}

.playlist-panel {
  box-sizing: border-box;
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  padding: 10px;
  border-radius: 16px;
  clip-path: inset(0 round 16px);
  background: var(--bg-panel);
}

.playlist-widget.is-ready {
  opacity: 1;
}

.widget-header {
  display: flex;
  flex: 0 0 38px;
  align-items: center;
  gap: 9px;
  min-width: 0;
  margin-bottom: 8px;
}

.queue-search {
  display: flex;
  flex: 0 0 34px;
  align-items: center;
  gap: 8px;
  min-width: 0;
  margin-bottom: 6px;
  padding: 0 9px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-muted);
  background: var(--bg-base);
}

.queue-search:focus-within {
  border-color: var(--accent-purple);
}

.queue-search input {
  width: 100%;
  min-width: 0;
  border: 0;
  outline: 0;
  color: var(--text-primary);
  background: transparent;
  font-size: 11px;
}

.queue-search input::placeholder {
  color: var(--text-muted);
}

.widget-icon {
  display: grid;
  flex: 0 0 34px;
  width: 34px;
  height: 34px;
  place-items: center;
  border-radius: 9px;
  color: var(--accent-purple);
  background: var(--accent-gradient-soft);
}

.widget-title {
  flex: 1;
  min-width: 0;
}

.title {
  overflow: hidden;
  font-size: 12.5px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle {
  margin-top: 2px;
  color: var(--text-secondary);
  font-size: 10.5px;
}

.icon-button {
  display: grid;
  flex: 0 0 28px;
  width: 28px;
  height: 28px;
  place-items: center;
  border: 0;
  border-radius: 7px;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
}

.icon-button:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.widget-error {
  flex: 0 0 auto;
  margin: 0 0 6px;
  color: var(--color-error, #ff6b81);
  font-size: 11px;
}

.queue {
  display: flex;
  flex: 1 1 0;
  flex-direction: column;
  gap: 3px;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 2px;
  scrollbar-gutter: stable;
}

.queue::-webkit-scrollbar {
  width: 7px;
}

.queue::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: var(--border-strong);
}

.queue-row {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 56px;
  padding: 5px 6px;
  border: 1px solid transparent;
  border-radius: 8px;
  color: var(--text-primary);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.queue-row:hover {
  background: var(--bg-hover);
}

.queue-row.current {
  border-color: var(--border-strong);
  background: var(--accent-gradient-soft);
}

.queue-index {
  flex: 0 0 20px;
  color: var(--text-muted);
  font-size: 10px;
  text-align: center;
}

.queue-thumbnail {
  flex: 0 0 42px;
  width: 42px;
  height: 42px;
  overflow: hidden;
  border-radius: 6px;
  object-fit: cover;
}

.queue-thumbnail-placeholder {
  display: grid;
  place-items: center;
  color: var(--text-secondary);
  background: var(--bg-elevated);
}

.queue-row.current .queue-index {
  color: var(--accent-purple);
}

.queue-meta {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.queue-title,
.queue-artist {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue-title {
  font-size: 11.5px;
  font-weight: 600;
}

.queue-artist,
.queue-time {
  color: var(--text-muted);
  font-size: 10px;
}

.queue-time {
  flex: 0 0 auto;
  font-variant-numeric: tabular-nums;
}

.queue-empty {
  display: grid;
  flex: 1;
  place-items: center;
  color: var(--text-muted);
  font-size: 11px;
}
</style>
