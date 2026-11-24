<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  ListMusic,
  Maximize2,
  Clapperboard,
  Music2,
  Pause,
  Play,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
  X,
} from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useSettingsStore } from "../stores/settings";
import { api, events, onEvent } from "../services/tauri";
import { formatDuration } from "../types/media";

const VIDEO_WIDGET_LABEL = "video-widget";
const PLAYLIST_WIDGET_LABEL = "playlist-widget";
type DetachedWidgetLabel = typeof VIDEO_WIDGET_LABEL | typeof PLAYLIST_WIDGET_LABEL;

const library = useLibraryStore();
const player = usePlayerStore();
const settings = useSettingsStore();
const widgetWindow = getCurrentWindow();

const ready = ref(false);
const volumeSliderVisible = ref(false);
const videoWidgetOpen = ref(false);
const playlistWidgetOpen = ref(false);
const openingVideoWidget = ref(false);
const openingPlaylistWidget = ref(false);
const widgetError = ref("");
let unlistenVideoWidget: (() => void) | null = null;
let unlistenPlaylistWidget: (() => void) | null = null;
let activeDragPointer: number | null = null;
let dragState: {
  pointerId: number;
  startScreenX: number;
  startScreenY: number;
  startPosition: PhysicalPosition;
  scaleFactor: number;
} | null = null;
let pendingDragPosition: PhysicalPosition | null = null;
let dragAnimationFrame = 0;

const current = computed(() => player.state.current);
const playing = computed(() => player.state.status === "playing");
const hasTrack = computed(() => !!current.value);
const isVideo = computed(() => current.value?.kind === "video");
const artwork = computed(() => library.thumbnailUrl(current.value?.thumbnailPath ?? null));
const subtitle = computed(() => {
  const item = current.value;
  if (!item) return "Nothing playing";
  return [item.artist, item.album].filter(Boolean).join(" · ") || item.kind;
});
const progress = computed(() =>
  player.state.durationMs > 0
    ? (player.state.positionMs / player.state.durationMs) * 100
    : 0,
);
const widgetOpacityStyle = computed(() => {
  const minimum = Math.max(0, Math.min(100, settings.settings.playerWidgetMinOpacity));
  const maximum = Math.max(
    minimum,
    Math.max(0, Math.min(100, settings.settings.playerWidgetMaxOpacity)),
  );
  return {
    "--widget-min-opacity": String(minimum / 100),
    "--widget-max-opacity": String(maximum / 100),
  };
});
onMounted(() => {
  // The shared index.html splash belongs to the main window only.
  document.getElementById("boot-splash")?.remove();
  ready.value = true;
  // Keep this window transparent so it blends with the desktop.
  void getCurrentWebview().setBackgroundColor([0, 0, 0, 0]).catch(() => {});

  void onEvent<boolean>(events.videoWidgetVisibilityChanged, (visible) => {
    videoWidgetOpen.value = visible;
    if (!visible) {
      void positionWidgetStack(false, playlistWidgetOpen.value).catch((error: unknown) => {
        widgetError.value = String(error);
      });
    }
  }).then((unlisten) => {
    unlistenVideoWidget = unlisten;
  }).catch((error: unknown) => {
    widgetError.value = String(error);
  });
  void onEvent<boolean>(events.playlistWidgetVisibilityChanged, (visible) => {
    playlistWidgetOpen.value = visible;
    if (!visible) {
      void positionWidgetStack(videoWidgetOpen.value, false).catch((error: unknown) => {
        widgetError.value = String(error);
      });
    }
  }).then((unlisten) => {
    unlistenPlaylistWidget = unlisten;
  }).catch((error: unknown) => {
    widgetError.value = String(error);
  });

  void (async () => {
    // Reveal first: a slow IPC call must never leave the widget unseen.
    await widgetWindow.show().catch(() => {});

    await Promise.allSettled([settings.initialize(), player.initialize()]);

    // A fast off/on toggle can hide the window while this webview was still
    // loading. Only keep it visible if the setting is still enabled on disk.
    const currentSettings = await api.getSettings().catch(() => null);
    if (currentSettings && !currentSettings.playerWidget) {
      await widgetWindow.hide().catch(() => {});
      return;
    }

    // Return activation to the main window so its custom title-bar controls
    // keep responding after this always-on-top window is shown.
    const main = await WebviewWindow.getByLabel("main");
    await main?.setFocus().catch(() => {});

  })();
});

onBeforeUnmount(() => {
  unlistenVideoWidget?.();
  unlistenPlaylistWidget?.();
  if (dragAnimationFrame) cancelAnimationFrame(dragAnimationFrame);
  player.dispose();
  settings.disposeListeners();
});

function createWidgetWindow(
  label: DetachedWidgetLabel,
  title: string,
  width: number,
  height: number,
  transparent: boolean,
): Promise<WebviewWindow> {
  return new Promise((resolve, reject) => {
    const child = new WebviewWindow(label, {
      url: "index.html",
      title,
      width,
      height,
      minWidth: 320,
      minHeight: 240,
      center: true,
      decorations: false,
      transparent,
      shadow: false,
      alwaysOnTop: true,
      skipTaskbar: true,
      resizable: true,
      visible: false,
      focus: false,
    });
    void child.once("tauri://created", () => resolve(child)).catch(reject);
    void child.once("tauri://error", (event) => reject(new Error(String(event.payload)))).catch(reject);
  });
}

async function getWidgetWindow(
  label: DetachedWidgetLabel,
  title: string,
  width: number,
  height: number,
  transparent: boolean,
): Promise<WebviewWindow> {
  const existing = await WebviewWindow.getByLabel(label);
  return existing ?? await createWidgetWindow(label, title, width, height, transparent);
}

async function positionWidgetStack(showVideo: boolean, showPlaylist: boolean) {
  const [playerPosition, playerSize, monitor, videoWindow, playlistWindow] = await Promise.all([
    widgetWindow.outerPosition(),
    widgetWindow.outerSize(),
    currentMonitor(),
    showVideo ? WebviewWindow.getByLabel(VIDEO_WIDGET_LABEL) : Promise.resolve(null),
    showPlaylist ? WebviewWindow.getByLabel(PLAYLIST_WIDGET_LABEL) : Promise.resolve(null),
  ]);
  if (!monitor) throw new Error("Could not determine the PlayerWidget monitor.");
  if (showVideo && !videoWindow) throw new Error("VideoWidget window is unavailable.");
  if (showPlaylist && !playlistWindow) throw new Error("PlaylistWidget window is unavailable.");

  const area = monitor.workArea;
  const areaRight = area.position.x + area.size.width;
  const areaBottom = area.position.y + area.size.height;
  const margin = Math.round(16 * monitor.scaleFactor);
  const clamp = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), Math.max(min, max));
  const playerX = clamp(
    playerPosition.x,
    area.position.x,
    areaRight - playerSize.width,
  );
  const rightEdge = playerX + playerSize.width;

  const videoSize =
    showVideo && videoWindow ? await videoWindow.outerSize() : null;
  let playlistSize =
    showPlaylist && playlistWindow ? await playlistWindow.outerSize() : null;
  if (playlistSize && playlistWindow) {
    const minPlaylistHeight = Math.round(240 * monitor.scaleFactor);
    const availablePlaylistHeight =
      area.size.height -
      margin * 2 -
      playerSize.height -
      (videoSize?.height ?? 0);
    const fittedHeight = Math.max(
      minPlaylistHeight,
      Math.min(playlistSize.height, availablePlaylistHeight),
    );
    if (fittedHeight !== playlistSize.height) {
      await playlistWindow.setSize(new PhysicalSize(playlistSize.width, fittedHeight));
      playlistSize = await playlistWindow.outerSize();
    }
  }

  const stackHeight =
    playerSize.height + (videoSize?.height ?? 0) + (playlistSize?.height ?? 0);
  const stackTop = Math.max(
    area.position.y + margin,
    areaBottom - margin - stackHeight,
  );
  const playerY = stackTop + (videoSize?.height ?? 0);
  const positionX = (width: number) =>
    clamp(rightEdge - width, area.position.x, areaRight - width);

  await widgetWindow.setPosition(new PhysicalPosition(playerX, playerY));
  if (videoSize && videoWindow) {
    await videoWindow.setPosition(
      new PhysicalPosition(positionX(videoSize.width), stackTop),
    );
  }
  if (playlistSize && playlistWindow) {
    await playlistWindow.setPosition(
      new PhysicalPosition(
        positionX(playlistSize.width),
        playerY + playerSize.height,
      ),
    );
  }
}

async function toggleVideoWidget() {
  if (openingVideoWidget.value) return;
  openingVideoWidget.value = true;
  widgetError.value = "";
  try {
    const child = await WebviewWindow.getByLabel(VIDEO_WIDGET_LABEL);
    if (videoWidgetOpen.value) {
      await api.setWidgetVideo(false);
      await child?.hide();
      videoWidgetOpen.value = false;
      await positionWidgetStack(false, playlistWidgetOpen.value);
      return;
    }
    const videoWindow = await getWidgetWindow(VIDEO_WIDGET_LABEL, "Slide Up Video", 320, 240, true);
    await positionWidgetStack(true, playlistWidgetOpen.value);
    await videoWindow.show();
    await api.setWidgetVideo(true);
    videoWidgetOpen.value = true;
  } catch (error) {
    widgetError.value = String(error);
  } finally {
    openingVideoWidget.value = false;
  }
}

async function togglePlaylistWidget() {
  if (openingPlaylistWidget.value) return;
  openingPlaylistWidget.value = true;
  widgetError.value = "";
  try {
    if (playlistWidgetOpen.value) {
      const child = await WebviewWindow.getByLabel(PLAYLIST_WIDGET_LABEL);
      await child?.hide();
      playlistWidgetOpen.value = false;
      await positionWidgetStack(videoWidgetOpen.value, false);
      return;
    }
    const playlistWindow = await getWidgetWindow(
      PLAYLIST_WIDGET_LABEL,
      "Slide Up Playlist",
      320,
      480,
      true,
    );
    await positionWidgetStack(videoWidgetOpen.value, true);
    await playlistWindow.show();
    playlistWidgetOpen.value = true;
  } catch (error) {
    widgetError.value = String(error);
  } finally {
    openingPlaylistWidget.value = false;
  }
}

async function togglePlayback() {
  if (!hasTrack.value) return;
  await player.togglePause();
}

async function openMainWindow() {
  const main = await WebviewWindow.getByLabel("main");
  if (!main) return;
  await main.unminimize().catch(() => {});
  await main.show().catch(() => {});
  await main.setFocus().catch(() => {});
}

async function closeWidget() {
  try {
    const currentSettings = await api.getSettings();
    if (currentSettings.playerWidget) {
      await api.updateSettings({ ...currentSettings, playerWidget: false });
    }
  } finally {
    // Hide, do not destroy: recreating the WebView2 window on every toggle
    // is what causes the CPU/RAM spike.
    await widgetWindow.hide().catch(() => {});
  }
}

function onDragStart(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, textarea, a, label, [data-no-drag]")) return;
  event.preventDefault();

  activeDragPointer = event.pointerId;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  void (async () => {
    const [startPosition, scaleFactor] = await Promise.all([
      widgetWindow.outerPosition(),
      widgetWindow.scaleFactor(),
    ]);
    if (activeDragPointer !== event.pointerId) return;
    dragState = {
      pointerId: event.pointerId,
      startScreenX: event.screenX,
      startScreenY: event.screenY,
      startPosition,
      scaleFactor,
    };
  })().catch((error: unknown) => {
    activeDragPointer = null;
    dragState = null;
    widgetError.value = String(error);
  });
}

function onDragMove(event: PointerEvent) {
  const state = dragState;
  if (!state || state.pointerId !== event.pointerId) return;
  pendingDragPosition = new PhysicalPosition(
    Math.round(state.startPosition.x + (event.screenX - state.startScreenX) * state.scaleFactor),
    Math.round(state.startPosition.y + (event.screenY - state.startScreenY) * state.scaleFactor),
  );
  if (dragAnimationFrame) return;
  dragAnimationFrame = requestAnimationFrame(() => {
    dragAnimationFrame = 0;
    const position = pendingDragPosition;
    if (!position) return;
    void widgetWindow.setPosition(position).catch((error: unknown) => {
      widgetError.value = String(error);
    });
  });
}

function onDragEnd(event: PointerEvent) {
  if (activeDragPointer !== event.pointerId) return;
  activeDragPointer = null;
  dragState = null;
  if (dragAnimationFrame) {
    cancelAnimationFrame(dragAnimationFrame);
    dragAnimationFrame = 0;
  }
  const position = pendingDragPosition;
  pendingDragPosition = null;
  if (position) {
    void widgetWindow.setPosition(position).catch((error: unknown) => {
      widgetError.value = String(error);
    });
  }
}

function onSeek(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  void player.seek(value);
}

function onVolumeWheel(event: WheelEvent) {
  const delta = event.deltaY > 0 ? -0.05 : 0.05;
  void player.setVolume(player.state.volume + delta);
}

function toggleVolumeSlider() {
  volumeSliderVisible.value = !volumeSliderVisible.value;
}
</script>

<template>
  <div
    class="widget"
    :class="{ 'is-ready': ready }"
    :style="widgetOpacityStyle"
    @pointerdown="onDragStart"
    @pointermove="onDragMove"
    @pointerup="onDragEnd"
    @pointercancel="onDragEnd"
  >
    <div class="panel">
      <header class="widget-header">
        <div class="art" :class="{ placeholder: !artwork }">
          <img v-if="artwork" :src="artwork" alt="" draggable="false" />
          <Music2 v-else :size="18" />
        </div>

        <div class="meta">
          <div class="title" :title="current?.title">{{ current?.title || "Slide Up" }}</div>
          <div class="subtitle" :title="subtitle">{{ subtitle }}</div>
        </div>

        <div class="header-actions">
          <button class="icon-button" title="Open full window" @click="openMainWindow">
            <Maximize2 :size="14" />
          </button>
          <button class="icon-button" title="Close widget" @click="closeWidget">
            <X :size="15" />
          </button>
        </div>
      </header>

      <div class="timeline">
        <span class="time">{{ formatDuration(player.state.positionMs) }}</span>
        <input
          class="seek"
          type="range"
          min="0"
          :max="Math.max(player.state.durationMs, 1)"
          :value="player.state.positionMs"
          :style="{ '--fill': `${progress}%` }"
          :disabled="!hasTrack"
          aria-label="Seek"
          @change="onSeek"
        />
        <span class="time">{{ formatDuration(player.state.durationMs) }}</span>
      </div>

      <div class="controls">
        <div class="control-group volume-controls">
          <button
            class="icon-button volume"
            :title="volumeSliderVisible ? 'Hide volume controls' : 'Show volume controls'"
            aria-label="Volume controls"
            aria-controls="player-volume-slider"
            :aria-expanded="volumeSliderVisible"
            @click="toggleVolumeSlider"
            @wheel.prevent="onVolumeWheel"
          >
            <VolumeX v-if="player.state.muted || player.state.volume === 0" :size="15" />
            <Volume2 v-else :size="15" />
          </button>
          <input
            v-if="volumeSliderVisible"
            id="player-volume-slider"
            class="volume-slider"
            type="range"
            min="0"
            max="1"
            step="0.01"
            :value="player.state.muted ? 0 : player.state.volume"
            :style="{ '--fill': `${(player.state.muted ? 0 : player.state.volume) * 100}%` }"
            aria-label="Volume"
            title="Volume"
            @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
          />
          <button
            v-if="volumeSliderVisible"
            class="icon-button volume-mute"
            :title="player.state.muted ? 'Unmute' : 'Mute'"
            :aria-label="player.state.muted ? 'Unmute' : 'Mute'"
            @click="player.toggleMute()"
          >
            <VolumeX v-if="player.state.muted || player.state.volume === 0" :size="14" />
            <Volume2 v-else :size="14" />
          </button>
        </div>
        <div class="control-group transport-controls">
          <button class="icon-button" title="Previous" :disabled="!hasTrack" @click="player.previous()">
            <SkipBack :size="16" fill="currentColor" />
          </button>
          <button class="play-button" :title="playing ? 'Pause' : 'Play'" :disabled="!hasTrack" @click="togglePlayback">
            <Pause v-if="playing" :size="18" fill="currentColor" />
            <Play v-else :size="18" fill="currentColor" />
          </button>
          <button class="icon-button" title="Next" :disabled="!hasTrack" @click="player.next()">
            <SkipForward :size="16" fill="currentColor" />
          </button>
        </div>
        <div class="control-group widget-controls">
          <button
            v-if="isVideo"
            class="icon-button"
            :class="{ active: videoWidgetOpen }"
            :title="videoWidgetOpen ? 'Hide video window' : 'Open video window'"
            :aria-pressed="videoWidgetOpen"
            :disabled="openingVideoWidget"
            @click="toggleVideoWidget"
          >
            <Clapperboard :size="16" />
          </button>
          <button
            class="icon-button queue-toggle"
            :class="{ active: playlistWidgetOpen }"
            :title="playlistWidgetOpen ? 'Hide playlist window' : 'Open playlist window'"
            :aria-pressed="playlistWidgetOpen"
            :disabled="openingPlaylistWidget"
            @click="togglePlaylistWidget"
          >
            <ListMusic :size="16" />
            <span v-if="player.queue.length" class="queue-count">{{ player.queue.length }}</span>
          </button>
        </div>
      </div>
      <p v-if="widgetError" class="widget-error" role="alert">{{ widgetError }}</p>
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

.widget {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 16px;
  background: transparent;
  color: var(--text-primary);
  opacity: 0;
  user-select: none;
  cursor: grab;
}

.widget:active {
  cursor: grabbing;
}

.widget.is-ready {
  opacity: var(--widget-min-opacity, 0.1);
}

.widget.is-ready:hover {
  opacity: var(--widget-max-opacity, 1);
}

.panel {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  width: 100%;
  height: 100%;
  gap: 5px;
  min-height: 0;
  overflow: hidden;
  padding: 8px 12px;
  border-radius: 16px;
  background: var(--bg-panel);
}

.widget-header {
  display: flex;
  flex: 0 0 32px;
  align-items: center;
  gap: 9px;
  min-width: 0;
}

.art {
  flex: 0 0 32px;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: 9px;
  color: var(--text-secondary);
  background: var(--accent-gradient-soft);
}

.art img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.meta {
  flex: 1;
  min-width: 0;
}

.title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  font-weight: 650;
  line-height: 1.15;
}

.subtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 1px;
  color: var(--text-secondary);
  font-size: 10px;
  line-height: 1.15;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 3px;
}

.controls {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  min-width: 0;
}

.control-group {
  display: flex;
  align-items: center;
  gap: 3px;
  min-width: 0;
}

.volume-controls {
  justify-self: start;
}

.transport-controls {
  justify-self: center;
}

.widget-controls {
  justify-self: end;
}

.icon-button {
  display: grid;
  place-items: center;
  width: 27px;
  height: 27px;
  border: 0;
  border-radius: 7px;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
}

.icon-button:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.icon-button:disabled,
.play-button:disabled {
  opacity: 0.4;
  cursor: default;
}

.volume {
  width: 22px;
}

.volume-slider {
  flex: 0 1 42px;
  width: 42px;
  min-width: 32px;
  height: 4px;
  margin: 0 1px;
  appearance: none;
  border-radius: var(--radius-full);
  background: linear-gradient(
    to right,
    var(--accent-purple) 0%,
    var(--accent-blue) var(--fill, 0%),
    var(--border-subtle) var(--fill, 0%),
    var(--border-subtle) 100%
  );
  cursor: pointer;
}

.volume-slider::-webkit-slider-thumb {
  width: 10px;
  height: 10px;
  appearance: none;
  border: 0;
  border-radius: 50%;
  background: var(--text-primary);
}

.volume-slider::-moz-range-thumb {
  width: 10px;
  height: 10px;
  border: 0;
  border-radius: 50%;
  background: var(--text-primary);
}

.play-button {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: 50%;
  color: #fff;
  background: var(--accent-gradient);
  cursor: pointer;
}

.queue-toggle {
  position: relative;
}

.icon-button.active {
  color: var(--accent-purple);
  background: var(--accent-gradient-soft);
}

.widget-error {
  margin: 0;
  color: var(--color-error, #ff6b81);
  font-size: 10px;
  line-height: 1.3;
}

.queue-count {
  position: absolute;
  top: -2px;
  right: -3px;
  min-width: 14px;
  padding: 0 3px;
  border-radius: var(--radius-full);
  color: #fff;
  background: var(--accent-purple);
  font-size: 9px;
  line-height: 14px;
  text-align: center;
}

.timeline {
  display: flex;
  flex: 0 0 12px;
  align-items: center;
  gap: 7px;
}

.time {
  flex: 0 0 34px;
  color: var(--text-muted);
  font-size: 9.5px;
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.seek {
  flex: 1;
  height: 4px;
  margin: 0;
  appearance: none;
  border-radius: var(--radius-full);
  background: linear-gradient(
    to right,
    var(--accent-purple) 0%,
    var(--accent-blue) var(--fill, 0%),
    var(--border-subtle) var(--fill, 0%),
    var(--border-subtle) 100%
  );
  cursor: pointer;
}

.seek:disabled {
  cursor: default;
  opacity: 0.6;
}

.seek::-webkit-slider-thumb {
  appearance: none;
  width: 10px;
  height: 10px;
  border: 0;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 0 0 2px var(--accent-purple);
}

.seek::-moz-range-thumb {
  width: 10px;
  height: 10px;
  border: 0;
  border-radius: 50%;
  background: #fff;
}

</style>
