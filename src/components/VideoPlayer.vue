<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  Play, Pause, SkipBack, SkipForward, Volume2, Volume1, VolumeX, Maximize, Minimize,
  Captions, Shuffle, Repeat, Repeat1,
  PictureInPicture2, Check, Settings, ChevronDown, ChevronLeft, ChevronRight, FolderOpen, StepForward, Camera, X, Sparkles, WandSparkles, Tv, Film, ListMusic,
} from "@lucide/vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api } from "../services/tauri";
import { usePlayerStore } from "../stores/player";
import { useLibraryStore } from "../stores/library";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "../composables/useToast";
import type { MediaItem } from "../types/media";
import type { MediaTrack } from "../types/player";
import AppLogo from "./AppLogo.vue";

const props = withDefaults(defineProps<{ queueOpen?: boolean }>(), {
  queueOpen: false,
});
const emit = defineEmits<{ togglePlaylist: [] }>();

const player = usePlayerStore();
const library = useLibraryStore();
const settings = useSettingsStore();
const toast = useToast();

const surface = ref<HTMLElement | null>(null);
const containerRef = ref<HTMLElement | null>(null);

// Controls visibility & auto-hide
const controlsVisible = ref(true);
const isInteractingWithControls = ref(false);
let idleTimer: number | undefined;
let syncTimer: number | undefined;
let resizeObserver: ResizeObserver | undefined;
let syncAttempts = 0;

// Popovers & Context Menu state
type ActiveMenu = "speed" | "settings" | null;
const activeMenu = ref<ActiveMenu>(null);

type SettingsSection = "root" | "speed" | "quality" | "aspect" | "subtitles" | "audio" | "filters" | "tools" | "ai";
const settingsSection = ref<SettingsSection>("root");

// Right-click context menu state
const showContextMenu = ref(false);
const contextMenuPos = ref({ x: 0, y: 0 });

const showVolumePopup = ref(false);
let volumeCloseTimer: number | undefined;

function openVolumePopup() {
  if (volumeCloseTimer) {
    window.clearTimeout(volumeCloseTimer);
    volumeCloseTimer = undefined;
  }
  showVolumePopup.value = true;
  wake();
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
  wake();
}

const isMenuOpen = computed(() => activeMenu.value !== null || showContextMenu.value || showVolumePopup.value);

function wake() {
  controlsVisible.value = true;
  if (idleTimer) {
    window.clearTimeout(idleTimer);
    idleTimer = undefined;
  }
  // Never hide if paused, or if hovering controls, or if any menu is open
  if (!player.isPlaying || isInteractingWithControls.value || isMenuOpen.value) {
    return;
  }
  idleTimer = window.setTimeout(() => {
    if (player.isPlaying && !isInteractingWithControls.value && !isMenuOpen.value) {
      controlsVisible.value = false;
    }
  }, 2800);
}

function onMouseLeave() {
  isInteractingWithControls.value = false;
  if (player.isPlaying && !isMenuOpen.value) {
    if (idleTimer) window.clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => {
      if (player.isPlaying && !isInteractingWithControls.value && !isMenuOpen.value) {
        controlsVisible.value = false;
      }
    }, 1200);
  }
}

function toggleMenu(menu: ActiveMenu) {
  activeMenu.value = activeMenu.value === menu ? null : menu;
  settingsSection.value = "root";
  showContextMenu.value = false;
  wake();
}

function closeMenus() {
  activeMenu.value = null;
  settingsSection.value = "root";
  showContextMenu.value = false;
  showVolumePopup.value = false;
  wake();
}

let clickTimer: number | undefined;

function onVideoClick() {
  // If context menu or settings/speed menu is open, click closes them
  if (isMenuOpen.value) {
    closeMenus();
    return;
  }

  // Handle single-click toggle play/pause vs double-click toggle fullscreen / exit pip
  if (clickTimer) {
    window.clearTimeout(clickTimer);
    clickTimer = undefined;
    if (player.isPip) {
      void player.togglePip();
    } else {
      toggleFullscreen();
    }
  } else {
    clickTimer = window.setTimeout(() => {
      clickTimer = undefined;
      player.togglePause();
      wake();
    }, 240);
  }
}

// Right-click context menu handler
function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  activeMenu.value = null;

  const container = containerRef.value;
  if (!container) return;
  const rect = container.getBoundingClientRect();

  let x = e.clientX - rect.left;
  let y = e.clientY - rect.top;

  // Clamp position within player container bounds (menu width ~224px, height ~390px)
  const menuW = 224;
  const menuH = 390;
  if (x + menuW > rect.width) x = Math.max(8, rect.width - menuW - 8);
  if (y + menuH > rect.height) y = Math.max(8, rect.height - menuH - 8);

  contextMenuPos.value = { x, y };
  showContextMenu.value = true;
  controlsVisible.value = true;
  if (idleTimer) {
    window.clearTimeout(idleTimer);
    idleTimer = undefined;
  }
}

function closeContextMenu() {
  showContextMenu.value = false;
  wake();
}

function handleGlobalClick(e: MouseEvent) {
  if (showContextMenu.value || activeMenu.value !== null || showVolumePopup.value) {
    const target = e.target as HTMLElement | null;
    if (
      target?.closest(".yt-context-menu") ||
      target?.closest(".yt-popover") ||
      target?.closest(".video-volume-popover-trigger") ||
      target?.closest(".volume-floating-panel")
    ) {
      return;
    }
    closeMenus();
  }
}

const current = computed(() => player.state.current);

const videoThumbnailUrl = computed(() => {
  const item = current.value
    ? library.byId(current.value.id) ?? current.value
    : null;
  return library.coverUrl(item);
});

const videoFormat = computed(() => {
  if (!current.value?.path) return "MP4";
  const ext = current.value.path.split(".").pop();
  return ext ? ext.toUpperCase() : "MP4";
});

const resolutionBadge = computed(() => {
  const h = player.state.videoHeight || 0;
  if (h >= 2160) return "4K";
  if (h >= 1080) return "FHD";
  if (h >= 720) return "HD";
  return "SD";
});

const speedOptions = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 4];
const aspectOptionList = [
  { label: "Fit Screen (Auto)", value: "default" },
  { label: "16:9 (Widescreen)", value: "16:9" },
  { label: "4:3 (Standard)", value: "4:3" },
  { label: "16:10 (Display)", value: "16:10" },
  { label: "2.35:1 (Cinema)", value: "2.35:1" },
];
const aspect = ref("default");

const currentSpeedLabel = computed(() =>
  player.state.speed === 1 ? "1x (Normal)" : `${player.state.speed}x`,
);
const currentAspectLabel = computed(
  () => aspectOptionList.find((opt) => opt.value === aspect.value)?.label ?? "Fit Screen (Auto)",
);

interface HlsVariant {
  name: string;
  resolution: string;
  bandwidth: number;
  url: string;
}

const hlsVariants = ref<HlsVariant[]>([]);
const currentMasterUrl = ref<string>("");
const activeHlsVariantUrl = ref<string | null>(null);

async function loadHlsVariants(url: string) {
  if (!url || (!url.includes(".m3u8") && !url.includes("m3u"))) {
    hlsVariants.value = [];
    currentMasterUrl.value = "";
    activeHlsVariantUrl.value = null;
    return;
  }
  if (hlsVariants.value.some((v) => v.url === url)) {
    activeHlsVariantUrl.value = url;
    return;
  }
  try {
    const res = await fetch(url);
    const text = await res.text();
    if (!text.includes("#EXT-X-STREAM-INF")) return;
    currentMasterUrl.value = url;
    activeHlsVariantUrl.value = null;
    const lines = text.split(/\r?\n/);
    const parsed: HlsVariant[] = [];
    let currentInf = "";
    for (const line of lines) {
      if (line.startsWith("#EXT-X-STREAM-INF:")) {
        currentInf = line;
      } else if (currentInf && line.trim() && !line.startsWith("#")) {
        const resMatch = currentInf.match(/RESOLUTION=(\d+x\d+)/i);
        const bwMatch = currentInf.match(/BANDWIDTH=(\d+)/i);
        const resolution = resMatch ? resMatch[1] : "";
        const h = resolution ? resolution.split("x")[1] : "";
        const bw = bwMatch ? parseInt(bwMatch[1], 10) : 0;
        const bwStr = bw >= 1000000 ? `${(bw / 1000000).toFixed(1)} Mbps` : bw > 0 ? `${Math.round(bw / 1000)} kbps` : "";
        const name = h ? `${h}p${bwStr ? ` (${bwStr})` : ""}` : bwStr || "Stream";

        let fullUrl = line.trim();
        if (!/^https?:\/\//i.test(fullUrl)) {
          const base = url.substring(0, url.lastIndexOf("/") + 1);
          fullUrl = new URL(fullUrl, base).href;
        }

        parsed.push({
          name,
          resolution,
          bandwidth: bw,
          url: fullUrl,
        });
        currentInf = "";
      }
    }
    parsed.sort((a, b) => b.bandwidth - a.bandwidth);
    hlsVariants.value = parsed;
  } catch {
    // Network or CORS fallback
  }
}

async function selectHlsVariant(variant: HlsVariant | null) {
  const target = variant ? variant.url : currentMasterUrl.value;
  if (!target || !current.value) return;
  activeHlsVariantUrl.value = variant ? variant.url : null;
  const pos = player.state.positionMs;
  const item: MediaItem = {
    id: current.value.id,
    path: target,
    kind: "video",
    title: current.value.title,
    artist: current.value.artist,
    album: current.value.album,
    durationMs: current.value.durationMs,
    codec: variant?.name ?? null,
    width: variant?.resolution ? parseInt(variant.resolution.split("x")[0], 10) : null,
    height: variant?.resolution ? parseInt(variant.resolution.split("x")[1], 10) : null,
    sizeBytes: 0,
    thumbnailPath: current.value.thumbnailPath,
    favorite: false,
    playCount: 0,
    lastPlayedAt: Date.now(),
    addedAt: Date.now(),
  };
  await player.play(item);
  if (pos > 1000) {
    window.setTimeout(() => void player.seek(pos).catch(() => {}), 500);
  }
}

const currentQualityLabel = computed(() => {
  if (hlsVariants.value.length > 0) {
    if (activeHlsVariantUrl.value) {
      const found = hlsVariants.value.find((v) => v.url === activeHlsVariantUrl.value);
      if (found) return found.name;
    }
    if (player.state.videoHeight) {
      return `Auto (${resolutionBadge.value})`;
    }
    return "Auto";
  }
  if (player.state.activeVideoTrack === -1) {
    if (player.state.videoHeight) {
      return `Auto (${resolutionBadge.value})`;
    }
    return "Auto";
  }
  const vid = player.state.videoTracks.find((track) => track.id === player.state.activeVideoTrack);
  if (vid) return vid.name;
  if (player.state.videoHeight) return `${player.state.videoHeight}p`;
  return "Auto";
});
const currentSubtitleLabel = computed(() => {
  if (player.state.activeSubtitleTrack === -1) return "Off";
  const sub = player.state.subtitleTracks.find((track) => track.id === player.state.activeSubtitleTrack);
  return sub?.name || `Sub ${player.state.activeSubtitleTrack}`;
});
const currentAudioLabel = computed(() => {
  if (player.state.activeAudioTrack === -1) return "Off (Mute)";
  const aud = player.state.audioTracks.find((track) => track.id === player.state.activeAudioTrack);
  return aud?.name || `Track ${player.state.activeAudioTrack}`;
});

const settingsTitle = computed(() => {
  switch (settingsSection.value) {
    case "quality": return "Quality";
    case "speed": return "Speed";
    case "aspect": return "Aspect Ratio";
    case "subtitles": return "Subtitles";
    case "audio": return "Audio Tracks";
    case "filters": return "Filters";
    case "tools": return "Tools";
    case "ai": return "AI Features";
    default: return "Settings";
  }
});

const settingsRows = computed(() => [
  { key: "ai" as const, label: "AI Features", value: player.aiEnhance ? (player.activeAiModelName || "Active") : (player.aiEnhanceAvailable ? "Off" : "Not setup") },
  { key: "quality" as const, label: "Quality", value: currentQualityLabel.value },
  { key: "speed" as const, label: "Speed", value: currentSpeedLabel.value },
  { key: "aspect" as const, label: "Aspect ratio", value: currentAspectLabel.value },
  { key: "audio" as const, label: "Audio tracks", value: currentAudioLabel.value },
  { key: "subtitles" as const, label: "Subtitles", value: currentSubtitleLabel.value },
  { key: "filters" as const, label: "Filters", value: "Brightness & more" },
  { key: "tools" as const, label: "Tools", value: "Frame & capture" },
]);

function openSettingsSection(section: SettingsSection) {
  settingsSection.value = section;
}

function backSettings() {
  settingsSection.value = "root";
}

type FilterKey = "videoBrightness" | "videoContrast" | "videoSaturation" | "videoGamma";

async function changeFilter(key: FilterKey, value: number) {
  void api.setVideoFilter(key.slice(5).toLowerCase(), value).catch(() => { });
  try {
    await settings.patch({ [key]: value } as Partial<typeof settings.settings>);
  } catch (error) {
    toast.error(String(error), "Could not save video filter");
  }
}

async function applyAutoFilters() {
  try {
    const filters = await api.autoVideoFilters();
    await settings.patch({
      videoBrightness: filters.brightness,
      videoContrast: filters.contrast,
      videoSaturation: filters.saturation,
      videoGamma: filters.gamma,
    });
    toast.success("Applied optimal video filters.");
  } catch (error) {
    toast.error(String(error), "Could not analyze the video frame");
  }
}

const volumeIcon = computed(() => {
  if (player.state.muted || player.state.volume === 0) return VolumeX;
  if (player.state.volume < 0.5) return Volume1;
  return Volume2;
});

const volumePercent = computed(() => {
  if (player.state.muted) return 0;
  return Math.round(player.state.volume * 100);
});

// Format time matching reference 00:12:34
function formatVideoTime(ms: number, totalMs: number = 0): string {
  if (!Number.isFinite(ms) || ms < 0) return "00:00";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (totalMs >= 3600000 || h > 0) {
    return `${pad(h)}:${pad(m)}:${pad(s)}`;
  }
  return `${pad(m)}:${pad(s)}`;
}

// Timeline interactive hover & seek tooltip
const isHoveringSeek = ref(false);
const hoverPixelX = ref(0);
const hoverTimeMs = ref(0);

// Cached seek-preview frames are generated once during library import. Hover
// only swaps an <img> source — no decoding, seeking or IPC per mousemove.
const STORYBOARD_FRAMES = 12;
const STORYBOARD_CHANGED_EVENT = "slideup:storyboard-changed";
const storyboardUrls = ref<Array<string | undefined>>([]);
let storyboardLoadedFor: number | null = null;

async function ensureStoryboard(force = false) {
  const id = player.state.current?.id;
  if (!id || (!force && storyboardLoadedFor === id)) return;
  storyboardLoadedFor = id;
  storyboardUrls.value = [];
  const urls = await Promise.all(
    Array.from({ length: STORYBOARD_FRAMES }, (_, index) =>
      api.getStoryboardFrame(id, index).catch(() => null),
    ),
  );
  if (storyboardLoadedFor !== id) return;
  storyboardUrls.value = urls.map((url) => url ?? undefined);
}

function onStoryboardChanged() {
  storyboardLoadedFor = null;
  void ensureStoryboard(true);
}

const hoverFrameUrl = computed(() => {
  const duration = player.state.durationMs;
  if (!storyboardUrls.value.length || duration <= 0) return undefined;
  const index = Math.min(
    STORYBOARD_FRAMES - 1,
    Math.max(0, Math.floor((hoverTimeMs.value / duration) * STORYBOARD_FRAMES)),
  );
  return storyboardUrls.value[index];
});

function onSeekMouseMove(e: MouseEvent) {
  const target = e.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
  hoverPixelX.value = x;
  if (player.state.durationMs > 0 && rect.width > 0) {
    const percent = x / rect.width;
    hoverTimeMs.value = Math.round(percent * player.state.durationMs);
    void ensureStoryboard();
  }
}

function onVolumeWheel(event: WheelEvent) {
  const delta = event.deltaY < 0 ? 0.05 : -0.05;
  const currentVol = player.state.muted ? 0 : player.state.volume;
  const newVol = Math.max(0, Math.min(1, currentVol + delta));
  void player.setVolume(Number(newVol.toFixed(2)));
}

const pipSizes = [
  { width: 320, height: 185, label: "S" },
  { width: 480, height: 275, label: "M" },
  { width: 640, height: 365, label: "L" },
];
const currentPipSizeIndex = ref(0);

async function cyclePipSize() {
  currentPipSizeIndex.value = (currentPipSizeIndex.value + 1) % pipSizes.length;
  const target = pipSizes[currentPipSizeIndex.value];
  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setSize(new LogicalSize(target.width, target.height));
    void nextTick(syncRect);
    window.setTimeout(syncRect, 60);
    window.setTimeout(syncRect, 180);
  } catch {
    // fallback
  }
}

let syncRaf = 0;
function syncRect() {
  if (syncRaf) cancelAnimationFrame(syncRaf);
  syncRaf = requestAnimationFrame(async () => {
    syncRaf = 0;
    const element = surface.value;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    if (rect.width < 2 || rect.height < 2) return;
    try {
      await api.syncVideoRect(
        Math.round(rect.left),
        Math.round(rect.top),
        Math.round(rect.width),
        Math.round(rect.height),
      );
    } catch {
      // Surface may not exist yet
    }
  });
}

function selectSpeed(s: number) {
  void player.setSpeed(s);
  closeMenus();
}

function selectAspect(val: string) {
  aspect.value = val;
  void player.setAspectRatio(val);
}

function selectVideoTrack(id: number) {
  void player.setVideoTrack(id);
}

function selectAudioTrack(id: number) {
  void player.setAudioTrack(id);
}

function selectSubtitleTrack(id: number) {
  void player.setSubtitleTrack(id);
}

function toggleLoop() {
  player.setRepeat(player.state.repeat === "off" ? "one" : "off");
}

function cycleSubtitle() {
  const list: MediaTrack[] = player.state.subtitleTracks;
  if (!list.length) {
    void player.openSubtitle();
    return;
  }
  const active = player.state.activeSubtitleTrack;
  const index = list.findIndex((track) => track.id === active);
  const next = list[(index + 1) % list.length];
  void player.setSubtitleTrack(next.id === active ? -1 : next.id);
}

async function takeScreenshot() {
  try {
    const path = await player.screenshot();
    toast.success(`Screenshot saved to ${path}`, "Captured");
  } catch (error) {
    toast.error(String(error), "Screenshot failed");
  }
}

async function toggleAiEnhance() {
  try {
    await player.toggleAiEnhance();
    wake();
  } catch (error) {
    // Keep the toggle silent by design; log failures for diagnostics only.
    console.warn("AI Enhance toggle failed", error);
  }
}

const upscaling = ref(false);

async function upscaleCurrentVideo() {
  const path = current.value?.path;
  if (!path || upscaling.value) return;
  upscaling.value = true;
  try {
    const output = await player.upscaleVideo(path);
    toast.success(`Upscaled video saved to ${output}`, "Real-ESRGAN");
  } catch (error) {
    toast.error(String(error), "Real-ESRGAN upscale failed");
  } finally {
    upscaling.value = false;
    wake();
  }
}

function toggleFullscreen() {
  void player.setFullscreen(!player.state.fullscreen);
}

function handleKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (showContextMenu.value) {
      closeContextMenu();
      return;
    }
    if (activeMenu.value !== null) {
      closeMenus();
      return;
    }
  }
}

watch(() => current.value?.id, () => {
  void nextTick(syncRect);
  syncAttempts = 0;
});

watch(
  () => current.value?.path,
  (newPath) => {
    if (newPath) {
      void loadHlsVariants(newPath);
    } else {
      hlsVariants.value = [];
    }
  },
  { immediate: true },
);

watch(() => [player.state.fullscreen, player.isPip] as const, () => {
  void nextTick(syncRect);
  window.setTimeout(syncRect, 60);
  window.setTimeout(syncRect, 200);
});

watch(() => player.state.status, () => {
  wake();
});

onMounted(() => {
  void getCurrentWebview().setBackgroundColor([0, 0, 0, 0]).catch(() => undefined);
  void api.setVideoLayer(false).catch(() => undefined);
  void api.setSurfaceVisible(true).catch(() => undefined);
  void player.refreshAiEnhanceAvailability();
  void nextTick(syncRect);
  wake();
  syncTimer = window.setInterval(() => {
    if (syncAttempts++ > 20) {
      if (syncTimer) window.clearInterval(syncTimer);
      return;
    }
    void syncRect();
  }, 100);
  if (surface.value && typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => void syncRect());
    resizeObserver.observe(surface.value);
  }
  window.addEventListener("resize", syncRect);
  window.addEventListener("keydown", handleKeyDown);
  window.addEventListener("click", handleGlobalClick);
  window.addEventListener(STORYBOARD_CHANGED_EVENT, onStoryboardChanged);
});

onBeforeUnmount(() => {
  if (syncRaf) cancelAnimationFrame(syncRaf);
  if (idleTimer) window.clearTimeout(idleTimer);
  if (clickTimer) window.clearTimeout(clickTimer);
  if (syncTimer) window.clearInterval(syncTimer);
  if (volumeCloseTimer) window.clearTimeout(volumeCloseTimer);
  resizeObserver?.disconnect();
  window.removeEventListener("resize", syncRect);
  window.removeEventListener("keydown", handleKeyDown);
  window.removeEventListener("click", handleGlobalClick);
  window.removeEventListener(STORYBOARD_CHANGED_EVENT, onStoryboardChanged);
  void api.setSurfaceVisible(false).catch(() => undefined);
});
</script>

<template>
  <div
    ref="containerRef"
    class="relative flex h-full w-full min-h-0 overflow-hidden select-none group"
    :class="'rounded-none'"
    @contextmenu.prevent="onContextMenu"
  >
    <!-- 1. Native Video Surface (Full container, libmpv renders directly here) -->
    <div
      ref="surface"
      class="absolute inset-0 h-full w-full z-0 pointer-events-none"
    />

    <!-- 2. Permanent Master Overlay Layer (Always captures mousemove & clicks over video) -->
    <!-- Center is transparent (rgba(0,0,0,0.001)), top and bottom are dimmed around controls -->
    <div
      class="absolute inset-0 z-20 flex flex-col justify-between select-none pointer-events-auto transition-[cursor] duration-150"
      :class="!controlsVisible && !isMenuOpen && player.isPlaying ? 'cursor-none' : 'cursor-default'"
      style="background-color: rgba(0, 0, 0, 0.001);"
      @mousemove="wake"
      @mouseenter="wake"
      @mouseleave="onMouseLeave"
      @click="onVideoClick"
      @contextmenu.prevent.stop="onContextMenu"
    >
      <!-- ================= Dedicated Custom Minimal PiP Mode ================= -->
      <template v-if="player.isPip">
        <div
          class="pip-overlay absolute inset-0 flex flex-col justify-between p-2.5 transition-opacity duration-300 pointer-events-none"
          :class="{
  'opacity-100': controlsVisible || !player.isPlaying,
  'opacity-0': !controlsVisible && player.isPlaying
}"
>
          <!-- Top Bar: Drag Region + Compact Title & Minimal Window Actions -->
          <div
data-tauri-drag-region
            class="pip-top-bar flex items-center justify-between w-full cursor-move rounded-lg px-2.5 py-1.5 bg-black/60 backdrop-blur-md border border-white/10 pointer-events-auto">
            <div data-tauri-drag-region class="flex items-center gap-1.5 min-w-0 pr-2">
              <span v-if="player.isLive" class="live-dot-badge scale-90 origin-left">LIVE</span>
              <span class="truncate font-semibold text-white/95 text-[11px] select-none">
                {{ current?.title ?? "Playing" }}
              </span>
            </div>

            <div class="flex items-center gap-1 shrink-0" style="-webkit-app-region: no-drag;">
              <button type="button" class="pip-btn text-[10px] font-bold px-1.5" title="Change PiP size"
                @click.stop="cyclePipSize">
                {{ pipSizes[currentPipSizeIndex].label }}
              </button>
              <button type="button" class="pip-btn" title="Expand to Full Player" @click.stop="player.togglePip()">
                <Maximize :size="12" />
              </button>
              <button type="button" class="pip-btn hover:text-red-400" title="Exit PiP"
                @click.stop="player.togglePip()">
                <X :size="12" />
              </button>
            </div>
          </div>

          <!-- Center: Minimal Quick Transport Controls -->
          <div data-tauri-drag-region
            class="flex items-center justify-center gap-4 my-auto select-none cursor-move pointer-events-auto">
            <button type="button" class="pip-circle-btn text-white/90 hover:text-white"
              style="-webkit-app-region: no-drag;" title="Previous" @click.stop="player.previous()">
              <SkipBack :size="14" fill="currentColor" />
            </button>

            <button type="button" class="pip-hero-btn" style="-webkit-app-region: no-drag;"
              :title="player.isPlaying ? 'Pause' : 'Play'" @click.stop="player.togglePause()">
              <Pause v-if="player.isPlaying" :size="18" fill="currentColor" />
              <Play v-else :size="18" fill="currentColor" class="ml-0.5" />
            </button>

            <button type="button" class="pip-circle-btn text-white/90 hover:text-white"
              style="-webkit-app-region: no-drag;" title="Next" @click.stop="player.next()">
              <SkipForward :size="14" fill="currentColor" />
            </button>
          </div>

          <!-- Bottom: Sleek Thin Progress Bar with Timestamps -->
          <div
            class="pip-bottom-bar flex flex-col gap-1 w-full px-2 py-1.5 rounded-lg bg-black/55 backdrop-blur-md border border-white/10 pointer-events-auto"
            style="-webkit-app-region: no-drag;" @click.stop>
            <div
              class="flex items-center justify-between text-[10px] font-semibold text-white/85 tabular-nums px-0.5 select-none">
              <span>{{ formatVideoTime(player.state.positionMs, player.state.durationMs) }}</span>
              <span>{{ formatVideoTime(player.state.durationMs, player.state.durationMs) }}</span>
            </div>
            <input class="pip-seek-bar w-full" type="range" min="0" :max="Math.max(player.state.durationMs, 1)"
              :value="player.state.positionMs" :style="{ '--fill': `${player.progress}%` }" aria-label="Seek"
              @input="player.seek(Number(($event.target as HTMLInputElement).value))" />
          </div>
        </div>
      </template>

      <!-- ================= Standard / Fullscreen mode Controls (When NOT in PiP) ================= -->
      <template v-else>
        <!-- Top Dimmed Controls Bar (Fades out when idle, dimmed only behind controls) -->
        <div class="video-top-overlay transition-opacity duration-300" :class="{
          'opacity-100 pointer-events-auto': controlsVisible || isMenuOpen || !player.isPlaying,
          'opacity-0 pointer-events-none': !controlsVisible && !isMenuOpen && player.isPlaying
        }" @click.stop @contextmenu.prevent.stop="onContextMenu" @mouseenter="isInteractingWithControls = true; wake()"
          @mouseleave="isInteractingWithControls = false; wake()">
          <!-- Left: Thumbnail & Title -->
          <div class="flex items-center gap-2.5 min-w-0">
            <!-- Channel Logo (IPTV) or Video File Thumbnail with TV/AppLogo fallback -->
            <div
              class="relative h-7 w-7 shrink-0 overflow-hidden rounded-md bg-black/50 border border-white/20 shadow-sm flex items-center justify-center">
              <img v-if="videoThumbnailUrl" :src="videoThumbnailUrl" :alt="current?.title ?? 'Thumbnail'"
                class="h-full w-full object-cover z-1"
                @error="($event.target as HTMLImageElement).style.display = 'none'" />
              <Tv v-if="player.isLive || (current?.id && current.id < -1000 && current?.kind !== 'audio')" :size="15"
                class="text-purple-400 opacity-90" />
              <AppLogo v-else-if="current?.kind === 'audio' || (current?.path && /\.(mp3|aac|m4a|flac|ogg|opus|wav|wma)(\?.*)?$/i.test(current.path))" class="h-4.5 w-4.5 opacity-85" />
              <Film v-else :size="13" class="text-white/60" />
            </div>

            <div class="min-w-0">
              <span class="truncate text-[13px] font-semibold text-white block drop-shadow-md">
                {{ current?.title ?? "Slide Up Video" }}
              </span>
              <span v-if="current?.artist" class="truncate text-[10.5px] text-white/70 block drop-shadow-sm">
                {{ current.artist }}
              </span>
            </div>
          </div>

          <!-- Right: Floating Action Buttons [Speed, CC, Settings, PiP, Fullscreen] -->
          <div class="flex items-center gap-1.5 shrink-0">
            <!-- Playlist Toggle Button (Replaces duplicate Speed button) -->
            <button
              type="button"
              class="ref-icon-btn"
              :class="{ 'ref-icon-btn-active': props.queueOpen }"
              :title="props.queueOpen ? 'Hide Playlist' : 'Show Playlist'"
              @click.stop="emit('togglePlaylist')"
            >
              <ListMusic :size="15" />
            </button>

            <!-- CC Subtitles Toggle -->
            <button
              type="button"
              class="ref-icon-btn text-xs font-bold"
              :class="{ 'ref-icon-btn-active': player.state.activeSubtitleTrack >= 0 }"
              title="Toggle Subtitles"
              @click.stop="cycleSubtitle"
            >
              <Captions :size="15" />
            </button>

            <!-- AI Enhance — available whenever any upscaler model/shader is installed -->
            <button
              v-if="player.aiEnhanceAvailable"
              type="button"
              class="ref-icon-btn"
              :class="{ 'ref-icon-btn-active': player.aiEnhance }"
              :title="player.aiEnhance ? `Disable AI Enhance (${player.activeAiModelName})` : `Enable AI Enhance (${player.activeAiModelName})`"
              @click.stop="toggleAiEnhance"
            >
              <Sparkles :size="15" />
            </button>

            <!-- Real-ESRGAN offline upscale — only when the model is installed -->
            <button v-if="player.realesrganAvailable" type="button" class="ref-icon-btn" :disabled="upscaling"
              :title="upscaling ? 'Upscaling video…' : 'Upscale this video with Real-ESRGAN'"
              @click.stop="upscaleCurrentVideo">
              <WandSparkles :size="15" />
            </button>

            <!-- Settings Button -->
            <button
              type="button"
              class="ref-icon-btn"
              :class="{ 'ref-icon-btn-active': activeMenu === 'settings' }"
              title="Player Settings"
              @click.stop="toggleMenu('settings')"
            >
              <Settings :size="15" />
            </button>

            <!-- Picture in Picture -->
            <button
              type="button"
              class="ref-icon-btn"
              :class="{ 'ref-icon-btn-active': player.isPip }"
              title="Picture in Picture"
              @click.stop="player.togglePip()"
            >
              <PictureInPicture2 :size="15" />
            </button>

            <!-- Fullscreen Toggle -->
            <button
              type="button"
              class="ref-icon-btn"
              :title="player.state.fullscreen ? 'Exit Fullscreen' : 'Fullscreen'"
              @click.stop="toggleFullscreen"
            >
              <Minimize v-if="player.state.fullscreen" :size="15" />
              <Maximize v-else :size="15" />
            </button>
          </div>
        </div>





 
      <!-- Center Area: Transparent, lets clicks bubble to master overlay for pause/play -->
      <div
        class="flex-1 w-full flex items-center justify-center pointer-events-none"
>


 
        <Transition name="fade-scale">
            <!-- Center loading / buffering indicator -->

 
          <div
              v-if="(player.state.status === 'loading' || (player.isLive && !player.isPlaying && player.state.positionMs === 0)) && !player.isPip"
              class="flex flex-col items-center gap-3 px-6 py-4 rounded-2xl bg-black/75 backdrop-blur-md border border-white/15 shadow-2xl pointer-events-none">
              <div class="relative flex items-center justify-center">
                <div class="h-11 w-11 rounded-full border-[3px] border-purple-500/30 border-t-purple-400 animate-spin">
                </div>
                <Tv v-if="player.isLive" :size="18" class="text-purple-400 absolute" />
              </div>
              <div class="flex items-center gap-2">
                <span class="h-2 w-2 rounded-full bg-purple-400 animate-pulse"></span>
                <span class="text-xs font-semibold text-white tracking-wide">
                  {{ player.isLive ? 'Connecting to live stream…' : 'Buffering stream…' }}
                </span>
              </div>
            </div>

            <!-- Center error message if stream fails -->
            <div v-else-if="player.state.status === 'error' && !player.isPip"
              class="flex flex-col items-center gap-2 px-5 py-3.5 rounded-2xl bg-black/80 backdrop-blur-md border border-red-500/40 text-red-200 shadow-2xl pointer-events-none max-w-sm text-center">
              <span class="text-xs font-bold text-red-400">Stream Playback Failed</span>
              <span class="text-[11px] text-white/80">{{ player.state.error || "Unable to play stream" }}</span>
            </div>

            <!-- Center play badge when paused -->
            <div v-else-if="!player.isPlaying && !player.isPip"
 
            class="h-16 w-16 rounded-full bg-black/60 backdrop-blur-md border border-white/20 flex items-center justify-center shadow-2xl transition-transform hover:scale-110 pointer-events-none"
          >
            <Play :size="28" fill="white" class="text-white ml-1" />
          </div>
        </Transition>
      </div>

      <!-- Bottom Dimmed Controls Bar (Fades out when idle, dimmed only behind controls) -->
      <div
        class="video-bottom-overlay transition-opacity duration-300"
        :class="{
  'opacity-100 pointer-events-auto': controlsVisible || isMenuOpen || !player.isPlaying,
  'opacity-0 pointer-events-none': !controlsVisible && !isMenuOpen && player.isPlaying
        }"
        @click.stop
        @contextmenu.prevent.stop="onContextMenu"
        @mouseenter="isInteractingWithControls = true; wake()"
        @mouseleave="isInteractingWithControls = false; wake()"
      >
        <!-- Row 1: Timeline Seek Bar with Exact Timestamps (00:12:34 --- 00:24:18) -->
        <div class="flex items-center gap-3 w-full">
          <span class="text-[12px] font-semibold tabular-nums text-white/90 shrink-0 select-none drop-shadow">
            {{ formatVideoTime(player.state.positionMs, player.state.durationMs) }}
          </span>

          <div
            class="relative flex-1 py-1 cursor-pointer flex items-center group"
            @mouseenter="isHoveringSeek = true"
            @mouseleave="isHoveringSeek = false"
            @mousemove="onSeekMouseMove"
          >
            <!-- Floating seek preview: cached frame + time (YouTube style) -->
            <div
              v-if="isHoveringSeek && player.state.durationMs > 0 && !player.isPip"
              class="pointer-events-none absolute -translate-x-1/2 flex flex-col items-center gap-1 rounded-lg bg-[#162040]/95 backdrop-blur-sm p-1.5 border border-[#2a3765] shadow-xl z-30"
              :class="hoverFrameUrl ? '-top-[138px]' : '-top-8'"
              :style="{ left: `${hoverPixelX}px` }"
            >
              <img
                v-if="hoverFrameUrl"
                :src="hoverFrameUrl"
                alt=""
                class="h-[90px] w-[160px] rounded object-cover bg-black/40"
              />
              <span class="text-[11px] font-bold tabular-nums text-white px-1">
                {{ formatVideoTime(hoverTimeMs, player.state.durationMs) }}
              </span>
            </div>

            <input
              class="ref-seek-bar w-full"
              type="range"
              min="0"
              :max="Math.max(player.state.durationMs, 1)"
              :value="player.state.positionMs"
              :style="{ '--fill': `${player.progress}%` }"
              aria-label="Seek video"
              @input="player.seek(Number(($event.target as HTMLInputElement).value))"
            />
          </div>

            <span v-if="player.isLive" class="live-dot-badge">
              <span class="live-dot"></span>
              LIVE
            </span>
            <span v-else class="text-[12px] font-semibold tabular-nums text-white/90 shrink-0 select-none drop-shadow">

 
            {{ formatVideoTime(player.state.durationMs, player.state.durationMs) }}
            </span>



            <!-- Volume with Vertical Slider Popover right next to seekbar -->
            <div class="relative flex items-center video-volume-popover-trigger shrink-0" @mouseenter="openVolumePopup"
              @mouseleave="scheduleCloseVolumePopup">

 
            <button
              type="button"
              class="ref-tool-btn"
:title="player.state.muted ? 'Unmute' : 'Volume'"
                @click.stop="toggleVolumePopup"
            >
              <component :is="volumeIcon" :size="19" class="text-white drop-shadow" />
            </button>

              <!-- Vertical Slider Popup -->
              <div v-if="showVolumePopup" class="volume-floating-panel" @mouseenter="openVolumePopup"
                @mouseleave="scheduleCloseVolumePopup" @wheel.prevent.stop="onVolumeWheel" @click.stop>
                <span class="volume-percentage">{{ volumePercent }}%</span>
                <div class="volume-slider-wrapper">

                 <input
class="su-range volume-vertical-range" type="range" min="0" max="1" step="0.01"
                    :value="player.state.muted ? 0 : player.state.volume" :style="{ '--fill': `${volumePercent}%` }"
                    aria-label="Volume" @input="player.setVolume(Number(($event.target as HTMLInputElement).value))" />

                </div>

             </div>
            </div>
          </div>

          <!-- Row 2: Controls (Left Spacer | Center Transport & Big Purple Play | Right Speed, Quality, Settings) -->
          <div class="flex items-center justify-between w-full mt-1.5 select-none">
            <!-- Left spacer to keep center transport buttons centered -->
            <div class="flex-1 flex items-center justify-start gap-2" />



 
          <!-- Center: Shuffle, Prev, Big Purple Play/Pause, Next, Repeat -->
            <div class="flex items-center gap-3 sm:gap-4 justify-center">

 
            <!-- Shuffle -->
            <button
              v-if="!player.isPip"
              type="button"
              class="ref-tool-btn"
              :class="{ active: player.state.shuffle }"
              title="Shuffle"
              @click.stop="player.setShuffle(!player.state.shuffle)"
            >
              <Shuffle :size="17" />
            </button>

            <!-- Previous -->
            <button
              type="button"
              class="ref-tool-btn"
              title="Previous"
              @click.stop="player.previous()"
            >
              <SkipBack :size="18" fill="currentColor" />
            </button>

            <!-- Hero Circular Purple Play/Pause Button -->
            <button
              type="button"
              class="ref-hero-play-btn"
              :title="player.isPlaying ? 'Pause' : 'Play'"
              @click.stop="player.togglePause()"
            >
              <Pause v-if="player.isPlaying" :size="20" fill="currentColor" />
              <Play v-else :size="20" fill="currentColor" class="ml-0.5" />
            </button>

            <!-- Next -->
            <button
              type="button"
              class="ref-tool-btn"
              title="Next"
              @click.stop="player.next()"
            >
              <SkipForward :size="18" fill="currentColor" />
            </button>

            <!-- Repeat -->
            <button
              v-if="!player.isPip"
              type="button"
              class="ref-tool-btn"
              :class="{ active: player.state.repeat !== 'off' }"
              :title="`Repeat: ${player.state.repeat}`"
              @click.stop="player.setRepeat(player.state.repeat === 'off' ? 'all' : player.state.repeat === 'all' ? 'one' : 'off')"
            >
              <Repeat1 v-if="player.state.repeat === 'one'" :size="17" />
              <Repeat v-else :size="17" />
            </button>
          </div>

          <!-- Right: 1.0x v Speed, Purple HD/4K Badge, Settings Gear -->
            <div class="flex-1 flex items-center justify-end gap-2">

 
            <!-- Speed selector: 1.0x v -->
            <button
              v-if="!player.isPip"
              type="button"
              class="ref-speed-btn"
              :class="{ active: activeMenu === 'speed' }"
              title="Playback Speed"
              @click.stop="toggleMenu('speed')"
            >
              <span>{{ player.state.speed }}x</span>
              <ChevronDown :size="12" class="opacity-80" />
            </button>

            <!-- Purple HD / 4K Badge -->
            <span class="ref-quality-badge">
              {{ resolutionBadge }}
            </span>

            <!-- Settings Gear -->
            <button
              v-if="!player.isPip"
              type="button"
              class="ref-tool-btn"
              :class="{ active: activeMenu === 'settings' }"
              title="Player Settings"
              @click.stop="toggleMenu('settings')"
            >
              <Settings :size="18" class="text-white drop-shadow" />
            </button>
          </div>
        </div>
      </div>
      </template>
    </div>

    <!-- 3. YouTube-style Floating Settings Popover (Above Settings Gear) -->
    <Transition name="fade-scale">
      <div
        v-if="activeMenu === 'settings'"
        class="yt-popover absolute bottom-16 right-6 w-64 z-40 pointer-events-auto"
        @click.stop
      >
        <div class="yt-popover-header flex items-center justify-between">
          <div class="flex items-center gap-1.5 font-bold text-white text-[11px] uppercase tracking-wider">
            <button
              v-if="settingsSection !== 'root'"
              type="button"
              class="h-5 w-5 flex items-center justify-center rounded hover:bg-white/10 text-white/70 hover:text-white cursor-pointer"
              @click="backSettings"
            >
              <ChevronLeft :size="13" />
            </button>
            <Settings v-else :size="13" class="text-[#8b5cf6]" />
            <span>{{ settingsTitle }}</span>
          </div>
          <button
            type="button"
            class="h-5 w-5 flex items-center justify-center rounded hover:bg-white/10 text-white/70 hover:text-white cursor-pointer"
            @click="closeMenus"
          >
            <X :size="12" />
          </button>
        </div>

        <!-- Root menu -->
        <div v-if="settingsSection === 'root'" class="p-1.5">
          <button
            v-for="row in settingsRows"
            :key="row.key"
            type="button"
            class="yt-list-item"
            @click="openSettingsSection(row.key)"
          >
            <span>{{ row.label }}</span>
            <span class="ml-auto mr-1 max-w-[88px] truncate text-[9.5px] text-white/45">{{ row.value }}</span>
            <ChevronRight :size="12" class="text-white/40" />
          </button>
        </div>

        <!-- Speed -->
        <div v-else-if="settingsSection === 'speed'" class="flex flex-col gap-0.5 p-1.5">
          <button
            v-for="s in speedOptions"
            :key="s"
            type="button"
            class="yt-list-item"
            :class="{ active: player.state.speed === s }"
            @click="selectSpeed(s)"
          >
            <span>{{ s }}x</span>
            <span v-if="s === 1" class="text-[9.5px] text-white/40 ml-1">(Normal)</span>
            <Check v-if="player.state.speed === s" :size="12" class="ml-auto text-[#8b5cf6]" />
          </button>
        </div>

        <!-- Aspect Ratio -->
        <div v-else-if="settingsSection === 'aspect'" class="p-2">
          <div class="flex flex-wrap gap-1">
            <button
              v-for="opt in aspectOptionList"
              :key="opt.value"
              type="button"
              class="yt-pill-btn"
              :class="{ active: aspect === opt.value }"
              @click="selectAspect(opt.value)"
            >
              <span>{{ opt.label }}</span>
              <Check v-if="aspect === opt.value" :size="11" class="text-[#8b5cf6]" />
            </button>
          </div>
        </div>

        <!-- Video Quality Selection -->
        <div v-else-if="settingsSection === 'quality'" class="p-2">
          <div
            class="flex items-center justify-between text-[10px] font-bold text-white/50 uppercase tracking-wider mb-1">
            <span>Video Quality</span>
            <span v-if="player.state.videoHeight" class="text-purple text-[10px]">{{ player.state.videoWidth }}x{{
              player.state.videoHeight }}</span>
          </div>
          <div class="flex flex-col gap-0.5 max-h-48 overflow-y-auto pr-1">
            <!-- HLS Adaptive Multi-Quality Variants -->
            <template v-if="hlsVariants.length > 0">
              <button
                type="button"
                class="yt-list-item"
                :class="{ active: activeHlsVariantUrl === null }"
                @click="selectHlsVariant(null)"
              >
                <span class="truncate">Auto {{ player.state.videoHeight ? `(${resolutionBadge})` : '' }}</span>
                <Check v-if="activeHlsVariantUrl === null" :size="12" class="text-[#8b5cf6]" />
              </button>
              <button
                v-for="v in hlsVariants"
                :key="v.url"
                type="button"
                class="yt-list-item"
                :class="{ active: activeHlsVariantUrl === v.url }"
                @click="selectHlsVariant(v)"
              >
                <span class="truncate">{{ v.name }}</span>
                <Check v-if="activeHlsVariantUrl === v.url" :size="12" class="text-[#8b5cf6]" />
              </button>
            </template>

            <!-- Standard Video Tracks / Renditions -->
            <template v-else>
              <!-- Auto option -->
              <button type="button" class="yt-list-item" :class="{ active: player.state.activeVideoTrack === -1 }"
                @click="selectVideoTrack(-1)">
                <span class="truncate">Auto {{ player.state.videoHeight ? `(${resolutionBadge})` : '' }}</span>
                <Check v-if="player.state.activeVideoTrack === -1" :size="12" class="text-[#8b5cf6]" />
              </button>

              <button v-for="vid in player.state.videoTracks" :key="vid.id" type="button" class="yt-list-item"
                :class="{ active: player.state.activeVideoTrack === vid.id }" @click="selectVideoTrack(vid.id)">
                <span class="truncate">{{ vid.name || `Quality ${vid.id}` }}</span>
                <Check v-if="player.state.activeVideoTrack === vid.id" :size="12" class="text-[#8b5cf6]" />
              </button>
              <div v-if="!player.state.videoTracks.length" class="text-[11px] text-white/50 py-2 text-center">
                Original Quality ({{ player.state.videoHeight ? `${player.state.videoHeight}p` : 'Default' }})
              </div>
            </template>
          </div>
        </div>

        <!-- Subtitles -->
        <div v-else-if="settingsSection === 'subtitles'" class="p-2">
          <div class="flex items-center justify-between text-[10px] font-bold text-white/50 uppercase tracking-wider mb-1">
            <span>Subtitles</span>
            <button
              type="button"
              class="text-[9.5px] text-[#a78bfa] hover:underline flex items-center gap-1 cursor-pointer font-medium"
              @click="player.openSubtitle()"
            >
              <FolderOpen :size="10" /> Load file
            </button>
          </div>
          <div class="flex flex-col gap-0.5 max-h-48 overflow-y-auto pr-1">
            <button
              type="button"
              class="yt-list-item"
              :class="{ active: player.state.activeSubtitleTrack === -1 }"
              @click="selectSubtitleTrack(-1)"
            >
              <span>Off</span>
              <Check v-if="player.state.activeSubtitleTrack === -1" :size="12" class="text-[#8b5cf6]" />
            </button>
            <button
              v-for="sub in player.state.subtitleTracks"
              :key="sub.id"
              type="button"
              class="yt-list-item"
              :class="{ active: player.state.activeSubtitleTrack === sub.id }"
              @click="selectSubtitleTrack(sub.id)"
            >
              <span class="truncate">{{ sub.name || `Sub ${sub.id}` }}</span>
              <Check v-if="player.state.activeSubtitleTrack === sub.id" :size="12" class="text-[#8b5cf6]" />
            </button>
          </div>
        </div>

        <!-- Audio Tracks -->
        <div v-else-if="settingsSection === 'audio'" class="p-2">
          <div class="flex flex-col gap-0.5 max-h-48 overflow-y-auto pr-1">
            <button
              type="button"
              class="yt-list-item"
              :class="{ active: player.state.activeAudioTrack === -1 }"
              @click="selectAudioTrack(-1)"
            >
              <span>Off (Mute)</span>
              <Check v-if="player.state.activeAudioTrack === -1" :size="12" class="text-[#8b5cf6]" />
            </button>
            <button
              v-for="aud in player.state.audioTracks"
              :key="aud.id"
              type="button"
              class="yt-list-item"
              :class="{ active: player.state.activeAudioTrack === aud.id }"
              @click="selectAudioTrack(aud.id)"
            >
              <span class="truncate">{{ aud.name || `Track ${aud.id}` }}</span>
              <Check v-if="player.state.activeAudioTrack === aud.id" :size="12" class="text-[#8b5cf6]" />
            </button>
          </div>
        </div>

        <!-- Filters -->
        <div v-else-if="settingsSection === 'filters'" class="flex flex-col gap-2 p-2.5">
          <button type="button"
            class="flex w-full items-center justify-center gap-1.5 rounded-md bg-[#8b5cf6]/20 px-2 py-1.5 text-[10.5px] font-semibold text-[#c4b5fd] hover:bg-[#8b5cf6]/30 cursor-pointer"
            @click="applyAutoFilters">
            <WandSparkles :size="12" /> Auto optimize
          </button>
          <div v-for="filter in ([
            { key: 'videoBrightness' as const, label: 'Brightness' },
            { key: 'videoContrast' as const, label: 'Contrast' },
            { key: 'videoSaturation' as const, label: 'Saturation' },
            { key: 'videoGamma' as const, label: 'Gamma' },
          ])" :key="filter.key" class="flex flex-col gap-1">
            <div class="flex items-center justify-between text-[10px] text-white/60">
              <span>{{ filter.label }}</span>
              <span class="tabular-nums text-white/85">{{ settings.settings[filter.key] }}</span>
            </div>
            <input type="range" min="-100" max="100" step="1" class="w-full accent-[#8b5cf6] cursor-pointer"
              :value="settings.settings[filter.key]"
              @input="changeFilter(filter.key, Number(($event.target as HTMLInputElement).value))" />
          </div>
          <button type="button" class="text-center text-[9.5px] text-white/40 hover:text-white/70 cursor-pointer"
            @click="settings.patch({ videoBrightness: 0, videoContrast: 0, videoSaturation: 0, videoGamma: 0 })">Reset
            filters</button>
        </div>

        <!-- Tools -->
        <div v-else-if="settingsSection === 'tools'" class="flex items-center justify-between p-2 text-[10.5px]">
          <button
            type="button"
            class="flex items-center gap-1 px-2 py-1 rounded bg-white/5 hover:bg-white/10 text-white cursor-pointer"
            @click="player.frameStep()"
          >
            <StepForward :size="12" /> Next Frame
          </button>
          <button
            type="button"
            class="flex items-center gap-1 px-2 py-1 rounded bg-white/5 hover:bg-white/10 text-white cursor-pointer"
            @click="takeScreenshot"
          >
            <Camera :size="12" /> Screenshot
          </button>
        </div>

        <!-- AI Features Submenu -->
        <div v-else-if="settingsSection === 'ai'" class="flex flex-col gap-2 p-2.5 max-h-80 overflow-y-auto">
          <!-- Main AI Enhance Master Toggle -->
          <div class="flex items-center justify-between rounded-lg bg-white/5 p-2">
            <div class="flex flex-col">
              <div class="flex items-center gap-1.5 font-semibold text-[11px] text-white">
                <Sparkles :size="13" class="text-[#8b5cf6]" />
                <span>AI Video Enhance</span>
              </div>
              <span class="text-[9px] text-white/50">Real-time upscaling & debanding</span>
            </div>
            <button
              type="button"
              class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="player.aiEnhance ? 'bg-[#8b5cf6]' : 'bg-white/20'"
              @click="toggleAiEnhance"
            >
              <span
                class="pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow-lg ring-0 transition duration-200 ease-in-out"
                :class="player.aiEnhance ? 'translate-x-4' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Model Selection -->
          <div class="flex flex-col gap-1">
            <span class="text-[9.5px] font-bold text-white/40 uppercase tracking-wider px-1">AI Upscaler Model</span>
            <div class="flex flex-col gap-1">
              <button
                v-for="model in player.upscalerModels"
                :key="model.id"
                type="button"
                class="yt-list-item flex items-center justify-between !py-1.5 cursor-pointer"
                :class="{
                  active: player.activeAiModel === model.id && player.aiEnhance,
                  'opacity-40 cursor-not-allowed': !model.isDownloaded
                }"
                :disabled="!model.isDownloaded"
                @click="player.setActiveAiModel(model.id)"
              >
                <div class="flex flex-col text-left truncate mr-2">
                  <span class="text-[11px] text-white truncate">{{ model.name }}</span>
                  <span class="text-[8.5px] text-white/50">{{ model.speed }}</span>
                </div>
                <div class="flex items-center gap-1.5 shrink-0">
                  <span v-if="!model.isDownloaded" class="text-[8px] text-white/35 bg-white/5 px-1.5 py-0.5 rounded">Not installed</span>
                  <span v-else-if="player.activeAiModel === model.id" class="text-[8.5px] text-[#a78bfa] font-semibold">Active</span>
                  <Check v-if="player.activeAiModel === model.id && player.aiEnhance" :size="12" class="text-[#8b5cf6]" />
                </div>
              </button>
            </div>
          </div>

          <!-- Offline 4x Upscale Action -->
          <div v-if="player.realesrganAvailable" class="border-t border-white/10 pt-2">
            <button
              type="button"
              class="flex w-full items-center justify-center gap-1.5 rounded-md bg-[#8b5cf6]/20 px-2 py-1.5 text-[10.5px] font-semibold text-[#c4b5fd] hover:bg-[#8b5cf6]/30 cursor-pointer disabled:opacity-50"
              :disabled="upscaling"
              @click="upscaleCurrentVideo"
            >
              <WandSparkles :size="12" />
              <span>{{ upscaling ? 'Upscaling…' : 'Upscale Full Video (4x)' }}</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- 4. YouTube-style Floating Speed Popover (Above Bottom Speed Button) -->
    <Transition name="fade-scale">
      <div
        v-if="activeMenu === 'speed'"
        class="yt-popover absolute bottom-16 right-20 w-36 z-40 pointer-events-auto"
        @click.stop
      >
        <div class="yt-popover-header flex items-center justify-between">
          <span class="font-bold text-white text-[11px] uppercase tracking-wider">Speed</span>
          <button
            type="button"
            class="h-5 w-5 flex items-center justify-center rounded hover:bg-white/10 text-white/70 hover:text-white cursor-pointer"
            @click="closeMenus"
          >
            <X :size="12" />
          </button>
        </div>
        <div class="flex flex-col gap-0.5 p-1 max-h-48 overflow-y-auto">
          <button
            v-for="s in speedOptions"
            :key="s"
            type="button"
            class="yt-list-item"
            :class="{ active: player.state.speed === s }"
            @click="selectSpeed(s)"
          >
            <span>{{ s }}x</span>
            <span v-if="s === 1" class="text-[9.5px] text-white/40 ml-1">(Normal)</span>
            <Check v-if="player.state.speed === s" :size="12" class="text-[#8b5cf6]" />
          </button>
        </div>
      </div>
    </Transition>

    <!-- 5. Right-Click Video Context Menu (YouTube / VLC Desktop Style) -->
    <Transition name="fade-scale">
      <div
        v-if="showContextMenu"
        class="yt-context-menu"
        :style="{ left: `${contextMenuPos.x}px`, top: `${contextMenuPos.y}px` }"
        @click.stop
      >
        <!-- Play / Pause -->
        <button type="button" class="ctx-item" @click="player.togglePause(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <component :is="player.isPlaying ? Pause : Play" :size="14" class="text-[#a78bfa]" />
            <span>{{ player.isPlaying ? 'Pause' : 'Play' }}</span>
          </div>
          <span class="ctx-shortcut">Space</span>
        </button>

        <!-- Mute / Unmute -->
        <button type="button" class="ctx-item" @click="player.toggleMute(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <component :is="volumeIcon" :size="14" />
            <span>{{ player.state.muted ? 'Unmute' : 'Mute' }}</span>
          </div>
          <span class="ctx-shortcut">M</span>
        </button>

        <!-- Loop Toggle -->
        <button type="button" class="ctx-item" @click="toggleLoop(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <Repeat :size="14" :class="{ 'text-[#a78bfa]': player.state.repeat !== 'off' }" />
            <span>Loop</span>
          </div>
          <Check v-if="player.state.repeat !== 'off'" :size="13" class="text-[#8b5cf6]" />
        </button>

        <div class="ctx-divider" />

        <!-- Speed Quick Selector -->
        <div class="ctx-group">
          <div class="ctx-label">Speed</div>
          <div class="flex items-center gap-1 px-2 pb-1.5 pt-0.5">
            <button
              v-for="s in [0.5, 0.75, 1, 1.25, 1.5, 2]"
              :key="s"
              type="button"
              class="ctx-pill"
              :class="{ active: player.state.speed === s }"
              @click="selectSpeed(s); closeContextMenu()"
            >
              {{ s }}x
            </button>
          </div>
        </div>

        <!-- Aspect Ratio Quick Selector -->
        <div class="ctx-group">
          <div class="ctx-label">Aspect Ratio</div>
          <div class="flex items-center gap-1 px-2 pb-1.5 pt-0.5">
            <button
              v-for="opt in aspectOptionList"
              :key="opt.value"
              type="button"
              class="ctx-pill"
              :class="{ active: aspect === opt.value }"
              @click="selectAspect(opt.value); closeContextMenu()"
            >
              {{ opt.value === 'default' ? 'Fit' : opt.value }}
            </button>
          </div>
        </div>

        <!-- Quality / Video Tracks -->
        <div v-if="player.state.videoTracks.length > 0" class="ctx-group">
          <div class="ctx-label">Quality</div>
          <div class="flex items-center gap-1 px-2 pb-1.5 pt-0.5 overflow-x-auto">
            <button type="button" class="ctx-pill" :class="{ active: player.state.activeVideoTrack === -1 }"
              @click="selectVideoTrack(-1); closeContextMenu()">
              Auto
            </button>
            <button v-for="vid in player.state.videoTracks" :key="vid.id" type="button" class="ctx-pill"
              :class="{ active: player.state.activeVideoTrack === vid.id }"
              @click="selectVideoTrack(vid.id); closeContextMenu()">
              {{ vid.name || `Quality ${vid.id}` }}
            </button>
          </div>
        </div>

        <div class="ctx-divider" />

        <!-- Load Subtitles -->
        <button type="button" class="ctx-item" @click="player.openSubtitle(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <FolderOpen :size="14" />
            <span>Load Subtitles...</span>
          </div>
        </button>

        <!-- Take Screenshot -->
        <button type="button" class="ctx-item" @click="takeScreenshot(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <Camera :size="14" />
            <span>Take Screenshot</span>
          </div>
        </button>

        <!-- AI Enhance toggle & submenu shortcut -->
        <button
          v-if="player.aiEnhanceAvailable"
          type="button"
          class="ctx-item"
          @click="toggleAiEnhance(); closeContextMenu()"
        >
          <div class="flex items-center gap-2.5">
            <Sparkles :size="14" :class="{ 'text-[#a78bfa]': player.aiEnhance }" />
            <span>{{ player.aiEnhance ? `AI Enhance: ${player.activeAiModelName} (ON)` : 'Enable AI Enhance' }}</span>
          </div>
          <Check v-if="player.aiEnhance" :size="14" class="text-[#a78bfa]" />
        </button>

        <!-- Open AI Features submenu from context menu -->
        <button
          type="button"
          class="ctx-item"
          @click="toggleMenu('settings'); settingsSection = 'ai'; closeContextMenu()"
        >
          <div class="flex items-center gap-2.5">
            <Sparkles :size="14" class="text-white/60" />
            <span>AI Features Settings…</span>
          </div>
          <ChevronRight :size="12" class="text-white/40" />
        </button>

        <!-- Real-ESRGAN offline upscale -->
        <button v-if="player.realesrganAvailable" type="button" class="ctx-item" :disabled="upscaling"
          @click="upscaleCurrentVideo(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <WandSparkles :size="14" />
            <span>{{ upscaling ? 'Upscaling…' : 'Upscale Video (Real-ESRGAN)' }}</span>
          </div>
        </button>

        <div class="ctx-divider" />

        <!-- Picture in Picture -->
        <button type="button" class="ctx-item" @click="player.togglePip(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <PictureInPicture2 :size="14" :class="{ 'text-[#a78bfa]': player.isPip }" />
            <span>Picture in Picture</span>
          </div>
        </button>

        <!-- Fullscreen -->
        <button type="button" class="ctx-item" @click="toggleFullscreen(); closeContextMenu()">
          <div class="flex items-center gap-2.5">
            <component :is="player.state.fullscreen ? Minimize : Maximize" :size="14" />
            <span>{{ player.state.fullscreen ? 'Exit Fullscreen' : 'Fullscreen' }}</span>
          </div>
          <span class="ctx-shortcut">F</span>
        </button>

        <!-- Video stats footer -->
        <div class="ctx-divider" />
        <div class="px-3 py-1.5 text-[10px] text-white/45 flex items-center justify-between">
          <span>{{ resolutionBadge }} · {{ videoFormat }}</span>
          <span>{{ formatVideoTime(player.state.durationMs, player.state.durationMs) }}</span>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
/* Top Dimmed Controls Bar: gently dims only around top controls, transparent towards center */
.video-top-overlay,
.video-yt-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 1rem 1.25rem 2.25rem;
  background: linear-gradient(
    to bottom,
    rgba(0, 0, 0, 0.75) 0%,
    rgba(0, 0, 0, 0.38) 55%,
    rgba(0, 0, 0, 0.001) 100%
  );
  width: 100%;
}

/* Bottom Dimmed Controls Bar: gently dims only around bottom controls, transparent towards center */
.video-bottom-overlay,
.video-yt-bottom {
  display: flex;
  flex-direction: column;
  padding: 1.75rem 1.25rem 0.65rem;
  background: linear-gradient(
    to top,
    rgba(0, 0, 0, 0.92) 0%,
    rgba(0, 0, 0, 0.52) 60%,
    rgba(0, 0, 0, 0.001) 100%
  );
  width: 100%;
}

/* 4K Badge (top left) */
.ref-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 1.75rem;
  padding: 0 0.65rem;
  border-radius: 0.5rem;
  background: rgba(0, 0, 0, 0.65);
  border: 1px solid rgba(255, 255, 255, 0.18);
  backdrop-filter: blur(8px);
  color: #ffffff;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.05em;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
}

/* Top Square Icon Buttons [Speed, CC, Settings, PiP, Maximize] */
.ref-icon-btn {
  display: flex;
  height: 2rem;
  width: 2rem;
  align-items: center;
  justify-content: center;
  border-radius: 0.5rem;
  background: rgba(0, 0, 0, 0.65);
  border: 1px solid rgba(255, 255, 255, 0.18);
  backdrop-filter: blur(8px);
  color: #ffffff;
  cursor: pointer;
  transition: all var(--t-fast);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
}

.ref-icon-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  border-color: rgba(255, 255, 255, 0.35);
  color: #ffffff;
}

.ref-icon-btn-active {
  border-color: #8b5cf6 !important;
  color: #8b5cf6 !important;
  background: rgba(139, 92, 246, 0.25) !important;
}

/* Reference Timeline Seek Bar */
.ref-seek-bar {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 16px;
  margin: 0;
  background: transparent;
  cursor: pointer;
}

.ref-seek-bar::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: 9999px;
  background: linear-gradient(
    to right,
    #8b5cf6 0%,
    #8b5cf6 var(--fill, 0%),
    rgba(255, 255, 255, 0.28) var(--fill, 0%),
    rgba(255, 255, 255, 0.28) 100%
  );
}

.ref-seek-bar::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  margin-top: -5px;
  border-radius: 50%;
  background: #ffffff;
  border: 2.5px solid #8b5cf6;
  box-shadow: 0 0 8px rgba(139, 92, 246, 0.8);
  transition: transform 0.15s ease;
}

.ref-seek-bar:hover::-webkit-slider-thumb {
  transform: scale(1.3);
}

/* Reference Volume Bar & Vertical Popover */
.video-volume-popover-trigger {
  position: relative;
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
  border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.16));
  background: var(--bg-panel, rgba(14, 20, 42, 0.96));
  padding: 0.55rem 0.35rem 0.4rem;
  z-index: 100;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(16px);
}

.volume-percentage {
  font-size: 0.68rem;
  font-weight: 600;
  color: var(--text-primary, #ffffff);
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

.volume-floating-panel::after {
  content: '';
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  height: 16px;
}
.ref-vol-bar {
  -webkit-appearance: none;
  appearance: none;
  width: 76px;
  height: 16px;
  margin: 0;
  background: transparent;
  cursor: pointer;
}

.ref-vol-bar::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: 9999px;
  background: linear-gradient(
    to right,
    #8b5cf6 0%,
    #8b5cf6 var(--fill, 0%),
    rgba(255, 255, 255, 0.28) var(--fill, 0%),
    rgba(255, 255, 255, 0.28) 100%
  );
}

.ref-vol-bar::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 10px;
  height: 10px;
  margin-top: -3px;
  border-radius: 50%;
  background: #ffffff;
  border: 1.5px solid #8b5cf6;
  box-shadow: 0 0 4px rgba(0, 0, 0, 0.5);
}

/* Tool Buttons (Shuffle, Prev, Next, Repeat, Mute) */
.ref-tool-btn {
  display: flex;
  height: 1.95rem;
  width: 1.95rem;
  align-items: center;
  justify-content: center;
  border-radius: 0.5rem;
  color: rgba(255, 255, 255, 0.9);
  background: transparent;
  cursor: pointer;
  transition: all var(--t-fast);
}

.ref-tool-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  color: #ffffff;
}

.ref-tool-btn.active {
  color: #a78bfa;
}

/* Big Hero Play/Pause Button in Center */
.ref-hero-play-btn {
  display: flex;
  height: 2.45rem;
  width: 2.45rem;
  align-items: center;
  justify-content: center;
  border-radius: 9999px;
  background: #8b5cf6;
  color: #ffffff;
  box-shadow: 0 4px 14px rgba(139, 92, 246, 0.55);
  cursor: pointer;
  transition: all 0.15s ease;
}

.ref-hero-play-btn:hover {
  background: #7c3aed;
  transform: scale(1.08);
}

.ref-hero-play-btn:active {
  transform: scale(0.96);
}

/* Speed Button: 1.0x v */
.ref-speed-btn {
  display: flex;
  height: 1.85rem;
  align-items: center;
  gap: 0.25rem;
  padding: 0 0.5rem;
  border-radius: 0.45rem;
  font-size: 11.5px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9);
  background: transparent;
  cursor: pointer;
  transition: all var(--t-fast);
}

.ref-speed-btn:hover,
.ref-speed-btn.active {
  background: rgba(255, 255, 255, 0.15);
  color: #ffffff;
}

/* Purple Quality Badge (HD / 4K) */
.ref-quality-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.15rem 0.5rem;
  border-radius: 0.35rem;
  background: #8b5cf6;
  color: #ffffff;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  box-shadow: 0 2px 6px rgba(139, 92, 246, 0.4);
}

/* YouTube Floating Popover Menu */
.yt-popover {
  border-radius: 0.85rem;
  border: 1px solid rgba(255, 255, 255, 0.16);
  background: rgba(14, 20, 42, 0.96);
  backdrop-filter: blur(16px);
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.7);
  overflow: hidden;
}

.yt-popover-transparent {
  background: rgba(14, 20, 42, 0.5);
  backdrop-filter: blur(8px);
}
.yt-popover-header {
  padding: 0.5rem 0.75rem;
  background: rgba(255, 255, 255, 0.05);
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.yt-pill-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  border-radius: 0.35rem;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.05);
  padding: 0.2rem 0.5rem;
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.85);
  cursor: pointer;
  transition: all var(--t-fast);
}

.yt-pill-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #ffffff;
}

.yt-pill-btn.active {
  background: rgba(139, 92, 246, 0.3) !important;
  border-color: #8b5cf6 !important;
  color: #ffffff !important;
  font-weight: 600;
}

.yt-list-item {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  border-radius: 0.35rem;
  padding: 0.3rem 0.5rem;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.85);
  cursor: pointer;
  transition: all var(--t-fast);
}

.yt-list-item:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
}

.yt-list-item.active {
  background: rgba(139, 92, 246, 0.25);
  color: #ffffff;
  font-weight: 600;
}

/* Right-Click Video Context Menu (YouTube / VLC style) */
.yt-context-menu {
  position: absolute;
  z-index: 60;
  width: 220px;
  border-radius: 0.85rem;
  border: 1px solid rgba(255, 255, 255, 0.16);
  background: rgba(14, 20, 42, 0.96);
  backdrop-filter: blur(20px);
  box-shadow: 0 20px 45px rgba(0, 0, 0, 0.75);
  padding: 0.35rem;
  overflow: hidden;
}

.ctx-item {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  border-radius: 0.45rem;
  padding: 0.4rem 0.65rem;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.9);
  cursor: pointer;
  transition: all var(--t-fast);
}

.ctx-item:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #ffffff;
}

.ctx-shortcut {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.45);
  font-family: monospace;
}

.ctx-divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.1);
  margin: 0.35rem 0.25rem;
}

.ctx-group {
  display: flex;
  flex-direction: column;
}

.ctx-label {
  padding: 0.25rem 0.65rem 0.15rem;
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: rgba(255, 255, 255, 0.45);
}

.ctx-pill {
  flex: 1;
  text-align: center;
  padding: 0.2rem 0.35rem;
  border-radius: 0.35rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.8);
  cursor: pointer;
  transition: all var(--t-fast);
}

.ctx-pill:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #ffffff;
}

.ctx-pill.active {
  background: rgba(139, 92, 246, 0.3) !important;
  border-color: #8b5cf6 !important;
  color: #ffffff !important;
  font-weight: 600;
}

/* Animations */
.fade-scale-enter-active,
.fade-scale-leave-active {
  transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-scale-enter-from,
.fade-scale-leave-to {
  opacity: 0;
  transform: translateY(6px) scale(0.96);
}
/* Custom PiP Player Minimal Overlay Styles */
.pip-overlay {
  background: radial-gradient(circle at center,
      rgba(0, 0, 0, 0.08) 0%,
      rgba(0, 0, 0, 0.55) 100%);
}

.pip-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 1.45rem;
  min-width: 1.45rem;
  padding: 0 0.35rem;
  border-radius: 0.375rem;
  background: rgba(255, 255, 255, 0.14);
  border: 1px solid rgba(255, 255, 255, 0.18);
  color: #ffffff;
  cursor: pointer;
  transition: all 0.15s ease;
}

.pip-btn:hover {
  background: rgba(255, 255, 255, 0.28);
  border-color: rgba(255, 255, 255, 0.35);
}

.pip-circle-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 2rem;
  width: 2rem;
  border-radius: 9999px;
  background: rgba(0, 0, 0, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.22);
  backdrop-filter: blur(8px);
  cursor: pointer;
  transition: transform 0.15s ease, background 0.15s ease;
}

.pip-circle-btn:hover {
  transform: scale(1.12);
  background: rgba(255, 255, 255, 0.2);
}

.pip-hero-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 2.65rem;
  width: 2.65rem;
  border-radius: 9999px;
  background: #8b5cf6;
  border: 2px solid rgba(255, 255, 255, 0.35);
  color: #ffffff;
  box-shadow: 0 4px 14px rgba(139, 92, 246, 0.55);
  cursor: pointer;
  transition: transform 0.15s ease, background 0.15s ease;
}

.pip-hero-btn:hover {
  transform: scale(1.08);
  background: #7c3aed;
}

.pip-seek-bar {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 12px;
  margin: 0;
  background: transparent;
  cursor: pointer;
}

.pip-seek-bar::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 9999px;
  background: linear-gradient(to right,
      #8b5cf6 0%,
      #8b5cf6 var(--fill, 0%),
      rgba(255, 255, 255, 0.25) var(--fill, 0%),
      rgba(255, 255, 255, 0.25) 100%);
  transition: height 0.15s ease;
}

.pip-seek-bar:hover::-webkit-slider-runnable-track {
  height: 5px;
}

.pip-seek-bar::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 10px;
  height: 10px;
  margin-top: -3.5px;
  border-radius: 50%;
  background: #ffffff;
  border: 2px solid #8b5cf6;
  box-shadow: 0 0 6px rgba(139, 92, 246, 0.8);
  transition: transform 0.15s ease;
}

.pip-seek-bar:hover::-webkit-slider-thumb {
  transform: scale(1.25);
}
</style>