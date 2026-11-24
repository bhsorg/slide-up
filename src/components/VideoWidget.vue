<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { emit as emitAppEvent } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { X } from "@lucide/vue";
import { events, api } from "../services/tauri";
import { usePlayerStore } from "../stores/player";

const VIDEO_INSET = 0;
const VIDEO_CORNER_RADIUS = 14;
const videoWindow = getCurrentWindow();
const player = usePlayerStore();
const viewport = ref<HTMLElement | null>(null);
const ready = ref(false);
const errorMessage = ref("");
let observer: ResizeObserver | null = null;

function syncVideoRect() {
  const element = viewport.value;
  if (!element) return;
  const rect = element.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;
  void api
    .syncWidgetVideoRect(
      Math.round(rect.left + VIDEO_INSET),
      Math.round(rect.top + VIDEO_INSET),
      Math.round(rect.width - VIDEO_INSET * 2),
      Math.round(rect.height - VIDEO_INSET * 2),
      VIDEO_CORNER_RADIUS,
    )
    .catch((error: unknown) => {
      errorMessage.value = String(error);
    });
}

onMounted(async () => {
  document.getElementById("boot-splash")?.remove();
  void getCurrentWebview().setBackgroundColor([0, 0, 0, 0]).catch(() => {});
  try {
    await player.initialize();
    ready.value = true;
    await nextTick();
    observer = new ResizeObserver(syncVideoRect);
    if (viewport.value) {
      observer.observe(viewport.value);
    }
    await api.setWidgetVideo(true);
    syncVideoRect();
  } catch (error) {
    errorMessage.value = String(error);
  }
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
  void api.setWidgetVideo(false).catch(() => {});
  player.dispose();
});

async function closeWidget() {
  try {
    await api.setWidgetVideo(false);
    await emitAppEvent(events.videoWidgetVisibilityChanged, false);
    await videoWindow.hide();
  } catch (error) {
    errorMessage.value = String(error);
  }
}

function onDragStart(event: MouseEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, textarea, a, label, [data-no-drag]")) return;
  event.preventDefault();
  void videoWindow.startDragging().catch((error: unknown) => {
    errorMessage.value = String(error);
  });
}
</script>

<template>
  <div class="video-widget" :class="{ 'is-ready': ready }" @mousedown="onDragStart">
    <div ref="viewport" class="video-viewport">
      <button
        class="close-button"
        type="button"
        title="Close video window"
        aria-label="Close video window"
        @click.stop="closeWidget"
      >
        <X :size="16" />
      </button>
      <p v-if="errorMessage || player.state.error" class="widget-error" role="alert">
        {{ errorMessage || player.state.error }}
      </p>
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

.video-widget {
  box-sizing: border-box;
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  border-radius: 14px;
  background: transparent;
  color: var(--text-primary);
  opacity: 0;
}

.video-widget.is-ready {
  opacity: 1;
}

.video-viewport {
  position: absolute;
  inset: 0;
  overflow: hidden;
  border-radius: 14px;
  background: transparent;
}

.close-button {
  position: absolute;
  z-index: 1;
  top: 12px;
  right: 12px;
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border: 0;
  border-radius: 7px;
  color: var(--text-primary);
  background: var(--bg-panel);
  cursor: pointer;
  visibility: hidden;
  opacity: 0;
  pointer-events: none;
}

.video-widget:hover .close-button,
.close-button:focus-visible {
  visibility: visible;
  opacity: 1;
  pointer-events: auto;
}

.widget-error {
  position: absolute;
  z-index: 2;
  top: 12px;
  left: 12px;
  max-width: calc(100% - 56px);
  margin: 0;
  padding: 6px 8px;
  border-radius: 6px;
  color: var(--color-error, #ff6b81);
  background: var(--bg-panel);
  font-size: 11px;
}
</style>
