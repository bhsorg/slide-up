<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckCircle2, CircleAlert, Info, TriangleAlert, X } from "@lucide/vue";
import Sidebar from "./components/Sidebar.vue";
import Topbar from "./components/Topbar.vue";
import VideoPlayer from "./components/VideoPlayer.vue";
import AudioPlayer from "./components/AudioPlayer.vue";
import PlaylistPanel from "./components/PlaylistPanel.vue";
import PlaylistPicker from "./components/PlaylistPicker.vue";
import MediaProperties from "./components/MediaProperties.vue";
import ArtistBioModal from "./components/ArtistBioModal.vue";
import SplashScreen from "./components/SplashScreen.vue";
import PlaybackDock from "./components/PlaybackDock.vue";
import Home from "./views/Home.vue";
import MusicLibrary from "./views/MusicLibrary.vue";
import VideoLibrary from "./views/VideoLibrary.vue";
import Playlists from "./views/Playlists.vue";
import Favorites from "./views/Favorites.vue";
import Recent from "./views/Recent.vue";
import Settings from "./views/Settings.vue";
import LiveTv from "./views/LiveTv.vue";
import About from "./views/About.vue";
import { useLibraryStore } from "./stores/library";
import { usePlayerStore } from "./stores/player";
import { usePlaylistStore } from "./stores/playlist";
import { useSettingsStore } from "./stores/settings";
import { useToast } from "./composables/useToast";
import { useShortcuts } from "./composables/useShortcuts";
import { api, onEvent } from "./services/tauri";
import type { ViewId } from "./types/settings";

const library = useLibraryStore();
const player = usePlayerStore();
const playlists = usePlaylistStore();
const settings = useSettingsStore();
const toast = useToast();
const shortcuts = useShortcuts();

type Screen = ViewId | "player";
const view = ref<Screen>("home");
const queueOpen = ref(true);
const dragging = ref(false);
const windowFocused = ref(true);
const splashVisible = ref(true);
let unlistenDragDrop: (() => void) | null = null;
let unlistenFocus: (() => void) | null = null;
let unlistenTaskbar: (() => void) | null = null;

// Surface transparency and visibility management
async function applySurfaceMode() {
  try {
    await getCurrentWebview().setBackgroundColor([0, 0, 0, 0]).catch(() => undefined);
    await api.setVideoLayer(false).catch(() => undefined);
  } catch {
    // Surface may not exist yet
  }
}

const isLiveTrack = computed(() => {
  const current = player.state.current;
  if (!current) return false;
  return (
    player.isLive ||
    current.artist === "Live TV" ||
    current.album === "Live TV" ||
    (current.id && current.id < -1000)
  );
});

const sidebarView = computed<ViewId>(() => {
  if (view.value === "player") {
    if (isLiveTrack.value) return "tv";
    return player.isVideo ? "videos" : "music";
  }
  return view.value;
});
const isFullScreenVideo = computed(() => (player.state.fullscreen || player.isPip) && player.isVideo && view.value === "player");

// When a video is playing and the user switches away from the player view,
// the native video is rendered as a mini preview over the guitar card in the sidebar footer.
const miniVideo = computed(
  () => player.isVideo && view.value !== "player" && !isFullScreenVideo.value,
);

async function syncMiniVideo() {
  if (!miniVideo.value) return;
  const anchor = document.getElementById("mini-video-anchor");
  if (!anchor) return;
  const rect = anchor.getBoundingClientRect();
  if (rect.width < 2 || rect.height < 2) return;
  try {
    await api.setSurfaceVisible(true);
    await api.syncVideoRect(
      Math.round(rect.left),
      Math.round(rect.top),
      Math.round(rect.width),
      Math.round(rect.height),
    );
    await api.setVideoLayer(true);
  } catch {
    // Surface may not exist yet
  }
}

function onWindowResize() {
  if (miniVideo.value) {
    void syncMiniVideo();
  }
}

function navigate(next: Screen) {
  view.value = next;
  if (next !== "player" && !player.isPip) {
    if (player.isVideo) {
      void nextTick(syncMiniVideo);
    } else {
      void api.setSurfaceVisible(false).catch(() => undefined);
    }
  } else if (player.isVideo) {
    void api.setVideoLayer(false).catch(() => undefined);
    void api.setSurfaceVisible(true).catch(() => undefined);
  }
}

function openPlayer() {
  if (player.hasTrack) {
    view.value = "player";
    if (player.isVideo) {
      void api.setVideoLayer(false).catch(() => undefined);
      void api.setSurfaceVisible(true).catch(() => undefined);
    }
  }
}

let lastPlayedVideoId: number | null = null;

// Only automatically switch to player view when a NEW video begins playing
watch(
  () => [player.state.current?.id, player.isVideo] as const,
  ([id, isVideo]) => {
    if (id != null && isVideo && id !== lastPlayedVideoId) {
      lastPlayedVideoId = id;
      view.value = "player";
      void api.setVideoLayer(false).catch(() => undefined);
      void api.setSurfaceVisible(true).catch(() => undefined);
    } else if (!isVideo) {
      lastPlayedVideoId = null;
    }
  },
);

// Switch between full video player and mini-video over the sidebar guitar card
watch(
  miniVideo,
  async (active) => {
    await nextTick();
    if (active) {
      await syncMiniVideo();
    } else if (player.isVideo && view.value === "player") {
      await api.setVideoLayer(false).catch(() => undefined);
      await api.setSurfaceVisible(true).catch(() => undefined);
    } else if (!player.isVideo) {
      await api.setSurfaceVisible(false).catch(() => undefined);
    }
  },
);

let dragTimeout: number | undefined;

async function initializeDragDrop() {
  try {
    unlistenDragDrop = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        dragging.value = true;
        if (dragTimeout) window.clearTimeout(dragTimeout);
        dragTimeout = window.setTimeout(() => {
          dragging.value = false;
        }, 1500);
      } else if (event.payload.type === "drop") {
        dragging.value = false;
        if (dragTimeout) window.clearTimeout(dragTimeout);
        const paths = event.payload.paths;
        if (paths.length) {
          void library.importPaths(paths).then((result) => {
            if (result) toast.success(`Added ${result.added} item${result.added === 1 ? "" : "s"}.`);
          }).catch((error: unknown) => toast.error(String(error), "Drop import failed"));
        }
      } else {
        dragging.value = false;
        if (dragTimeout) window.clearTimeout(dragTimeout);
      }
    });
  } catch {
    // Drag-and-drop is a desktop-only feature.
  }
}

onMounted(async () => {
  const splashStartedAt = performance.now();
  // The inline HTML splash covered startup before Vue existed; the animated
  // SplashScreen component is now mounted, so hand over to it seamlessly.
  document.getElementById("boot-splash")?.remove();
  try {
    windowFocused.value = document.hasFocus();
    await applySurfaceMode();
    unlistenFocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      windowFocused.value = focused;
    });
    window.addEventListener("resize", onWindowResize);
    document.documentElement.dataset.theme = settings.theme;
    settings.initialize();
    if (settings.settings.startMinimized) {
      void getCurrentWindow().hide().catch(() => {});
    } else {
      void getCurrentWindow().show().catch(() => {});
    }
    toast.initialize();
    shortcuts.initialize();
    await Promise.allSettled([
      library.initialize(),
      player.initialize(),
      playlists.initialize(),
    ]);
    unlistenTaskbar = await onEvent<string>("taskbar-action", (action) => {
      if (action === "play_pause") player.togglePause();
      else if (action === "previous") player.previous();
      else if (action === "next") player.next();
    });
    await initializeDragDrop();
  } finally {
    // Keep the splash up briefly so it does not flash on very fast starts.
    const remaining = Math.max(0, 650 - (performance.now() - splashStartedAt));
    if (remaining > 0) {
      await new Promise((resolve) => window.setTimeout(resolve, remaining));
    }
    splashVisible.value = false;
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", onWindowResize);
  unlistenDragDrop?.();
  unlistenFocus?.();
  unlistenTaskbar?.();
  shortcuts.dispose();
  library.dispose();
  player.dispose();
  playlists.dispose();
  settings.disposeListeners();
});
</script>

<template>
  <div
    class="flex h-screen w-screen flex-col overflow-hidden text-ink"
    :class="view === 'player' && player.isVideo ? 'bg-transparent' : 'bg-base'"
  >
    <Topbar v-if="!isFullScreenVideo" @navigate="navigate" />

    <div
      class="flex min-h-0 flex-1 overflow-hidden"
      :class="view === 'player' && player.isVideo ? 'bg-transparent' : ''"
    >
      <Sidebar v-if="!isFullScreenVideo" :active="sidebarView" @navigate="navigate" />

      <main
        class="min-w-0 flex-1 overflow-hidden"
        :class="view === 'player' && player.isVideo ? 'bg-transparent' : 'bg-base'"
      >
        <VideoPlayer
          v-if="(view === 'player' || isFullScreenVideo) && player.isVideo"
          :queue-open="queueOpen"
          @toggle-playlist="queueOpen = !queueOpen"
        />
        <AudioPlayer v-else-if="view === 'player'" />
        <Home v-else-if="view === 'home'" @navigate="navigate" />
        <MusicLibrary v-else-if="view === 'music'" />
        <VideoLibrary v-else-if="view === 'videos'" />
        <Playlists v-else-if="view === 'playlists'" />
        <Favorites v-else-if="view === 'favorites'" />
        <Recent v-else-if="view === 'recent'" />
        <Settings v-else-if="view === 'settings'" @navigate="navigate" />
        <LiveTv v-else-if="view === 'tv'" @open-player="openPlayer" />
        <About v-else-if="view === 'about'" />
      </main>

      <PlaylistPanel v-if="!isFullScreenVideo" :open="queueOpen" @close="queueOpen = false" />
    </div>

    <!-- The playback dock is visible on all screens except full player or fullscreen video -->
    <PlaybackDock
      v-if="view !== 'player' && !isFullScreenVideo"
      :queue-open="queueOpen"
      @toggle-queue="queueOpen = !queueOpen"
      @open-player="openPlayer"
    />

    <PlaylistPicker />
    <MediaProperties />
    <ArtistBioModal />
    <SplashScreen :visible="splashVisible" />

    <div class="pointer-events-none fixed right-5 top-20 z-[60] flex w-[340px] flex-col gap-2">
      <TransitionGroup name="toast">
        <div
          v-for="item in toast.toasts"
          :key="item.id"
          class="pointer-events-auto flex items-start gap-3 rounded-xl border border-line-strong bg-elevated/95 p-3.5 shadow-[var(--shadow-panel)] backdrop-blur"
          :class="item.kind === 'error' ? 'border-l-[3px] border-l-[#ff5b8d]' : item.kind === 'success' ? 'border-l-[3px] border-l-[#35d39a]' : ''"
        >
          <component
            :is="item.kind === 'error' ? CircleAlert : item.kind === 'success' ? CheckCircle2 : item.kind === 'warning' ? TriangleAlert : Info"
            :size="17"
            class="mt-0.5 shrink-0"
            :class="item.kind === 'error' ? 'text-[#ff5b8d]' : item.kind === 'success' ? 'text-[#35d39a]' : 'text-purple-soft'"
          />
          <div class="min-w-0 flex-1">
            <div class="text-[12px] font-semibold text-ink">{{ item.title }}</div>
            <div class="mt-0.5 break-words text-[11px] leading-relaxed text-muted">{{ item.message }}</div>
          </div>
          <button type="button" class="text-faint hover:text-ink" @click="toast.dismiss(item.id)"><X :size="14" /></button>
        </div>
      </TransitionGroup>
    </div>

    <Transition name="toast">
      <div v-if="dragging" class="pointer-events-none fixed inset-5 z-[70] flex items-center justify-center rounded-2xl border-2 border-dashed border-purple bg-panel/95 shadow-2xl">
        <div class="p-6 text-center">
          <p class="text-sm font-semibold text-ink">Drop media to import</p>
          <p class="mt-1 text-xs text-muted">Audio and video files, or a folder</p>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.toast-enter-active, .toast-leave-active { transition: opacity var(--t-base), transform var(--t-base); }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateX(12px); }
.toast-move { transition: transform var(--t-base); }
</style>
