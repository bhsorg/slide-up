<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Play, Pause, SkipBack, SkipForward, Heart, ListPlus, ListMusic,
  Volume2, Volume1, VolumeX, Maximize, MicVocal, Tv
} from "@lucide/vue";
import { usePlayerStore } from "../stores/player";
import { useLibraryStore } from "../stores/library";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useLyrics } from "../composables/useLyrics";
import AppLogo from "./AppLogo.vue";
import { formatDuration } from "../types/media";

const props = defineProps<{ queueOpen: boolean }>();
const emit = defineEmits<{ toggleQueue: []; openPlayer: [] }>();

const player = usePlayerStore();
const library = useLibraryStore();
const toast = useToast();
const picker = usePlaylistPicker();
const {
  activeLine,
  showLyrics,
  setShowLyrics,
  load: loadLyrics,
  updatePosition: updateLyricPosition,
} = useLyrics();

watch(
  () => player.state.current?.id,
  () => {
    if (player.state.current && !player.isVideo) {
      void loadLyrics(player.state.current);
    }
  },
  { immediate: true },
);

watch(
  () => player.state.positionMs,
  (posMs) => {
    updateLyricPosition(posMs);
  },
);

function toggleLyrics() {
  setShowLyrics(!showLyrics.value);
  if (showLyrics.value) emit("openPlayer");
}

const showVolumePopup = ref(false);
let volumeCloseTimer: number | undefined;

function openVolumePopup() {
  if (volumeCloseTimer) {
    window.clearTimeout(volumeCloseTimer);
    volumeCloseTimer = undefined;
  }
  showVolumePopup.value = true;
}

function scheduleCloseVolumePopup() {
  volumeCloseTimer = window.setTimeout(() => {
    showVolumePopup.value = false;
  }, 250);
}

function toggleVolumePopup() {
  if (volumeCloseTimer) {
    window.clearTimeout(volumeCloseTimer);
    volumeCloseTimer = undefined;
  }
  showVolumePopup.value = !showVolumePopup.value;
}

function onVolumeWheel(event: WheelEvent) {
  const delta = event.deltaY < 0 ? 0.05 : -0.05;
  const currentVol = player.state.muted ? 0 : player.state.volume;
  const newVol = Math.max(0, Math.min(1, currentVol + delta));
  void player.setVolume(Number(newVol.toFixed(2)));
}

const current = computed(() => player.state.current);
const artwork = computed(() => library.coverUrl(current.value));
const favorite = computed(() => (current.value ? (library.byId(current.value.id)?.favorite ?? false) : false));

const volumeIcon = computed(() => {
  if (player.state.muted || player.state.volume === 0) return VolumeX;
  if (player.state.volume < 0.5) return Volume1;
  return Volume2;
});

async function toggleFavorite() {
  if (!current.value) return;
  const item = library.byId(current.value.id);
  if (!item) return;
  try {
    await library.toggleFavorite(item);
  } catch (error) {
    toast.error(String(error), "Could not update favorite");
  }
}

function addToPlaylist() {
  if (!current.value) return;
  const item = library.byId(current.value.id);
  if (!item) return;
  picker.askAddToPlaylist(item);
}
</script>

<template>
  <footer class="dock-container">
    <!-- Left: Track Details (matching slideup-ui.png) -->
    <div class="dock-left">
      <div class="dock-thumb" @click="emit('openPlayer')" title="Now Playing">
        <img v-if="artwork && current" :src="artwork" :alt="current.title" class="h-full w-full object-cover" />
        <div v-else class="flex h-full w-full items-center justify-center text-faint bg-linear-to-br from-purple/25 via-panel to-blue/20">
          <Tv v-if="player.isLive || (current?.id && current.id < -1000)" class="h-5 w-5 text-purple-400 opacity-90" />
          <AppLogo v-else class="h-6 w-6 opacity-80" />
        </div>
      </div>

      <div class="dock-meta" @click="emit('openPlayer')" title="Now Playing">
        <div class="dock-title flex items-center gap-1.5 min-w-0">
          <span class="truncate">{{ current?.title ?? "Beautiful Nature" }}</span>
        </div>
        <div v-if="activeLine?.text && !player.isVideo" class="dock-lyric truncate" title="Current lyrics">
          ♪ {{ activeLine.text }}
        </div>
        <div v-else class="dock-subtitle">{{ current?.artist || "Nature World" }}</div>
      </div>

      <button
        type="button"
        class="dock-icon-btn heart-btn"
        :class="{ active: favorite }"
        title="Favorite"
        @click="toggleFavorite"
      >
        <Heart :size="17" :fill="favorite ? 'currentColor' : 'none'" />
      </button>

      <button
        type="button"
        class="dock-icon-btn"
        title="Add to playlist…"
        @click="addToPlaylist"
      >
        <ListPlus :size="18" />
      </button>
    </div>

    <!-- Center: Transport Playback Controls (Previous, Big Circular Purple Play, Next) -->
    <div class="dock-center">
      <button
        type="button"
        class="dock-nav-btn"
        title="Previous track"
        @click="player.previous()"
      >
        <SkipBack :size="20" fill="currentColor" />
      </button>

      <button
        type="button"
        class="dock-play-circle"
        :title="player.isPlaying ? 'Pause' : 'Play'"
        @click="player.togglePause()"
      >
        <Pause v-if="player.isPlaying" :size="22" fill="currentColor" />
        <Play v-else :size="22" fill="currentColor" class="ml-0.5" />
      </button>

      <button
        type="button"
        class="dock-nav-btn"
        title="Next track"
        @click="player.next()"
      >
        <SkipForward :size="20" fill="currentColor" />
      </button>
    </div>

    <!-- Right: Timestamps, Progress Bar, Volume, Queue Badge & Queue Pill Button -->
    <div class="dock-right">
      <span v-if="player.isLive" class="live-dot-badge">
        <span class="live-dot"></span>
        LIVE
      </span>
      <span v-else class="dock-time">
        {{ formatDuration(player.state.positionMs) }} / {{ formatDuration(player.state.durationMs) }}
      </span>

      <!-- Mini Seek Slider -->
      <div class="dock-mini-seek">
        <input
          class="su-range w-36 sm:w-44"
          type="range"
          min="0"
          :max="Math.max(player.state.durationMs, 1)"
          :value="player.state.positionMs"
          :style="{ '--fill': `${player.progress}%` }"
          aria-label="Seek timeline"
          @input="player.seek(Number(($event.target as HTMLInputElement).value))"
        />
      </div>

      <!-- Volume Icon with reliable floating popover -->
      <div
        class="relative flex items-center"
        @mouseenter="openVolumePopup"
        @mouseleave="scheduleCloseVolumePopup"
      >
        <button
          type="button"
          class="dock-icon-btn"
          :class="{ active: showVolumePopup || player.state.muted }"
          title="Adjust volume (Click to open panel)"
          @click="toggleVolumePopup"
        >
          <component :is="volumeIcon" :size="18" />
        </button>

        <!-- Vertical Floating Volume Panel with hover bridge -->
        <div
          v-if="showVolumePopup"
          class="volume-floating-panel"
          @mouseenter="openVolumePopup"
          @mouseleave="scheduleCloseVolumePopup"
          @wheel.prevent="onVolumeWheel"
        >
          <!-- Percentage text on top -->
          <span class="volume-percentage">
            {{ player.state.muted ? '0%' : `${Math.round(player.state.volume * 100)}%` }}
          </span>

          <!-- Vertical slider wrapper -->
          <div class="volume-slider-wrapper">
            <input
              class="su-range volume-vertical-range"
              type="range"
              min="0"
              max="1"
              step="0.01"
              :value="player.state.muted ? 0 : player.state.volume"
              :style="{ '--fill': `${(player.state.muted ? 0 : player.state.volume) * 100}%` }"
              aria-label="Volume slider"
              @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
            />
          </div>

          <!-- Mute / Unmute Button at bottom -->
          <button
            type="button"
            class="dock-icon-btn h-7 w-7 text-muted hover:text-ink shrink-0"
            :title="player.state.muted ? 'Unmute' : 'Mute'"
            @click="player.toggleMute()"
          >
            <component :is="volumeIcon" :size="15" />
          </button>
        </div>
      </div>

      <!-- Lyrics quick toggle — opens Now Playing where the panel renders -->
      <button
        v-if="!player.isVideo"
        type="button"
        class="dock-icon-btn"
        :class="{ active: showLyrics }"
        :title="showLyrics ? 'Hide lyrics' : 'Show lyrics'"
        @click="toggleLyrics"
      >
        <MicVocal :size="18" />
      </button>

      <!-- Queue Icon Button with count badge only (matching clean compact requirement) -->
      <div class="relative flex items-center">
        <button
          type="button"
          class="dock-icon-btn relative"
          :class="{ active: props.queueOpen }"
          title="Toggle Playlist"
          @click="emit('toggleQueue')"
        >
          <ListMusic :size="18" />
          <span class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-purple px-1 text-[9px] font-bold text-white">
            {{ player.queue.length || 1 }}
          </span>
        </button>
      </div>

      <!-- Video Fullscreen quick toggle button -->
      <button
        v-if="player.isVideo"
        type="button"
        class="dock-icon-btn"
        :class="{ active: player.state.fullscreen }"
        title="Fullscreen"
        @click="player.setFullscreen(!player.state.fullscreen)"
      >
        <Maximize :size="18" />
      </button>
    </div>
  </footer>
</template>

<style scoped>
.dock-container {
  display: flex;
  height: var(--dock-height, 68px);
  width: 100%;
  flex-shrink: 0;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--border-subtle);
  background: var(--bg-dock);
  padding: 0 1.25rem;
  position: relative;
  z-index: 50;
}

.dock-left {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  width: 28%;
  min-width: 200px;
}

.dock-thumb {
  position: relative;
  height: 2.75rem;
  width: 2.75rem;
  flex-shrink: 0;
  overflow: hidden;
  border-radius: var(--radius-sm, 8px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  cursor: pointer;
  transition: transform var(--t-fast);
}
.dock-thumb:hover {
  transform: scale(1.03);
}

.dock-meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  cursor: pointer;
}

.dock-title {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  line-height: 1.25;
}

.dock-subtitle {
  font-size: 0.72rem;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 2px;
}

.dock-lyric {
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--accent-purple, #8b5cf6);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  margin-top: 2px;
}

.dock-center {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 1.25rem;
}

.dock-nav-btn {
  display: flex;
  height: 2rem;
  width: 2rem;
  align-items: center;
  justify-content: center;
  border-radius: 9999px;
  color: var(--text-secondary);
  transition: color var(--t-fast);
}
.dock-nav-btn:hover {
  color: var(--text-primary);
}

.dock-play-circle {
  display: flex;
  height: 2.6rem;
  width: 2.6rem;
  align-items: center;
  justify-content: center;
  border-radius: 9999px;
  background: var(--accent-purple);
  color: #ffffff;
  transition: transform var(--t-fast), opacity var(--t-fast);
}
.dock-play-circle:hover {
  transform: scale(1.05);
  opacity: 0.95;
}
.dock-play-circle:active {
  transform: scale(0.95);
}

.dock-right {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.75rem;
  width: 38%;
  min-width: 260px;
}

.dock-time {
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  white-space: nowrap;
}

.dock-mini-seek {
  display: flex;
  align-items: center;
}

.dock-icon-btn {
  display: flex;
  height: 2rem;
  width: 2rem;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm, 8px);
  color: var(--text-secondary);
  transition: all var(--t-fast);
}
.dock-icon-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}
.dock-icon-btn.active {
  color: var(--accent-purple);
  background: rgba(135, 63, 255, 0.12);
}

.heart-btn.active {
  color: var(--accent-purple);
}

.volume-floating-panel {
  position: absolute;
  bottom: calc(100% + 10px);
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.45rem;
  width: 42px;
  border-radius: var(--radius-md, 12px);
  border: 1px solid var(--border-strong);
  background: var(--bg-panel);
  padding: 0.55rem 0.35rem 0.4rem;
  z-index: 100;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}

.volume-percentage {
  font-size: 0.68rem;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  text-align: center;
  user-select: none;
  line-height: 1;
}

.volume-slider-wrapper {
  height: 96px;
  width: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
}

.volume-vertical-range {
  width: 96px !important;
  height: 16px;
  transform: rotate(-90deg);
  transform-origin: center center;
  cursor: pointer;
}

/* Invisible bridge so mouse moving from button to panel never triggers mouseleave */
.volume-floating-panel::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  height: 16px;
}
</style>
