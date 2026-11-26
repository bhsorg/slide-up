import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api, events, hasProgressShape, onEvent } from "../services/tauri";
import type { MediaItem } from "../types/media";
import { initialPlayerState } from "../types/player";
import type { MediaTrack, PlayMode, PlayerState, PlayerStatus, TrackInfo } from "../types/player";
import type { QueuePayload, TrackPayload } from "../services/tauri";

const emptyState = (): PlayerState => initialPlayerState();

export const usePlayerStore = defineStore("player", () => {
  const state = ref<PlayerState>(emptyState());
  const queue = ref<MediaItem[]>([]);
  const currentIndex = ref<number | null>(null);
  const queueName = ref("Queue");
  const queueOpen = ref(false);
  const isPip = ref(false);
  const aiEnhance = ref(false);
  const aiEnhanceAvailable = ref(false);
  const realesrganAvailable = ref(false);
  const activeAiModel = ref<string>("fsrcnnx-x2");
  const upscalerModels = ref<import("../types/model").UpscalerModelStatus[]>([]);
  const initialized = ref(false);
  const unlisteners: Array<() => void> = [];

  const isPlaying = computed(() => state.value.status === "playing");
  const isVideo = computed(() => state.value.current?.kind === "video");
  const activeAiModelName = computed(() => {
    const found = upscalerModels.value.find((m) => m.id === activeAiModel.value);
    return found ? found.name : "FSRCNNX x2";
  });
  const hasTrack = computed(() => !!state.value.current);
  const isLive = computed(() => {
    const current = state.value.current;
    if (!current) return false;
    if (current.artist === "Live TV" || current.album === "Live TV") return true;
    if (
      current.durationMs === 0 &&
      (current.path.startsWith("rtmp://") ||
        current.path.startsWith("rtsp://") ||
        (current.id && current.id < -1000))
    ) {
      return true;
    }
    return false;
  });
  const progress = computed(() =>
    state.value.durationMs > 0 ? (state.value.positionMs / state.value.durationMs) * 100 : 0,
  );

  function applyState(next: PlayerState) {
    state.value = { ...state.value, ...next };
  }

  function applyProgress(snapshot: { positionMs: number; durationMs: number; status?: PlayerStatus }) {
    state.value.positionMs = snapshot.positionMs;
    if (snapshot.durationMs > 0) state.value.durationMs = snapshot.durationMs;
    if (snapshot.status) state.value.status = snapshot.status;
  }

  function applyTracks(payload: TrackPayload) {
    if (payload.videoTracks) state.value.videoTracks = payload.videoTracks;
    state.value.audioTracks = payload.audioTracks;
    state.value.subtitleTracks = payload.subtitleTracks;
    if (typeof payload.activeVideoTrack === "number") state.value.activeVideoTrack = payload.activeVideoTrack;
    state.value.activeAudioTrack = payload.activeAudioTrack;
    state.value.activeSubtitleTrack = payload.activeSubtitleTrack;
  }

  function applyQueue(payload: QueuePayload) {
    queue.value = payload.queue;
    currentIndex.value = payload.currentIndex;
    queueName.value = payload.name;
  }

  /** Drop every piece of player data belonging to the previous workspace. */
  function clearLocal() {
    state.value = emptyState();
    queue.value = [];
    currentIndex.value = null;
    queueName.value = "";
    state.value.error = null;
  }

  /** Re-fetch player data from the current workspace without adding listeners. */
  async function reload() {
    try {
      const [snapshot, queueState, tracks] = await Promise.all([
        api.getSnapshot(),
        api.getQueue(),
        api.getTracks(),
      ]);
      applyState(snapshot);
      applyQueue(queueState);
      applyTracks(tracks);
    } catch (error) {
      state.value.error = error instanceof Error ? error.message : String(error);
    }
  }

  async function initialize() {
    if (initialized.value) return;
    initialized.value = true;
    await reload();

    unlisteners.push(
      await onEvent<PlayerState>(events.playerState, (payload) => applyState(payload)),
      await onEvent<unknown>(events.playerProgress, (payload) => {
        if (hasProgressShape(payload)) applyProgress(payload);
      }),
      await onEvent<TrackPayload>(events.playerTracks, (payload) => applyTracks(payload)),
      await onEvent<QueuePayload>(events.queueChanged, (payload) => applyQueue(payload)),
      await onEvent<MediaItem>("player://file-loaded", () => {
        refreshState();
      }),
      await onEvent<string>(events.playerError, (message) => {
        state.value.error = message;
        state.value.status = "error";
      }),
      await onEvent<boolean>("player://pip-changed", (active) => {
        isPip.value = active;
      }),
    );
    void refreshAiEnhanceAvailability();
  }

  function dispose() {
    unlisteners.forEach((unlisten) => unlisten());
    unlisteners.length = 0;
    initialized.value = false;
  }

  async function refreshState() {
    try {
      applyState(await api.getSnapshot());
    } catch {
      /* keep last known state */
    }
  }

  async function refreshQueue() {
    try {
      applyQueue(await api.getQueue());
    } catch {
      /* keep last known state */
    }
  }

  async function play(media: MediaItem) {
    state.value.error = null;
    state.value.status = "loading";
    state.value.current = media as TrackInfo;
    state.value.durationMs = media.durationMs;
    state.value.positionMs = 0;
    await api.playMedia(media);
    await refreshQueue();
  }

  async function playList(items: MediaItem[], index = 0, name = "Queue") {
    if (!items.length) return;
    state.value.error = null;
    state.value.status = "loading";
    state.value.current = items[index] as TrackInfo;
    state.value.durationMs = items[index].durationMs;
    state.value.positionMs = 0;
    await api.playQueue(items, index, name);
    await refreshQueue();
  }

  async function togglePause() {
    if (state.value.status === "playing") {
      state.value.status = "paused";
    } else if (state.value.status === "paused") {
      state.value.status = "playing";
    }
    await api.togglePause();
    await refreshState();
  }

  async function next() {
    await api.next();
    await refreshQueue();
  }

  async function previous() {
    await api.previous();
    await refreshQueue();
  }

  async function stop() {
    await api.stop();
    state.value.status = "stopped";
    state.value.current = null;
    state.value.positionMs = 0;
  }

  async function seek(ms: number) {
    const targetMs = state.value.durationMs > 0 ? Math.max(0, Math.min(ms, state.value.durationMs)) : Math.max(0, ms);
    state.value.positionMs = targetMs;
    try {
      await api.seek(Math.round(targetMs));
    } catch (e) {
      console.warn("Seek error ignored:", e);
    }
  }

  async function seekRelative(deltaMs: number) {
    try {
      await api.seekRelative(deltaMs);
    } catch (e) {
      console.warn("SeekRelative error ignored:", e);
    }
  }

  async function setVolume(volume: number) {
    const value = Math.max(0, Math.min(1, volume));
    state.value.volume = value;
    if (value > 0) state.value.muted = false;
    await api.setVolume(value);
  }

  async function setMuted(muted: boolean) {
    state.value.muted = muted;
    await api.setMuted(muted);
  }

  async function toggleMute() {
    await setMuted(!state.value.muted);
  }

  async function setSpeed(speed: number) {
    const value = Math.max(0.25, Math.min(4, speed));
    state.value.speed = value;
    await api.setSpeed(value);
  }

  async function setShuffle(shuffle: boolean) {
    state.value.shuffle = shuffle;
    await api.setShuffle(shuffle);
  }

  async function setRepeat(repeat: PlayMode) {
    state.value.repeat = repeat;
    await api.setRepeat(repeat);
  }

  async function setFullscreen(fullscreen: boolean) {
    state.value.fullscreen = fullscreen;
    await api.setFullscreen(fullscreen);
  }

  async function togglePip() {
    try {
      const active = await api.togglePip();
      isPip.value = active;
    } catch {
      isPip.value = false;
    }
  }

  async function setVideoTrack(id: number) {
    state.value.activeVideoTrack = id;
    await api.setVideoTrack(id);
  }

  async function setAudioTrack(id: number) {
    state.value.activeAudioTrack = id;
    await api.setAudioTrack(id);
  }

  async function setSubtitleTrack(id: number) {
    state.value.activeSubtitleTrack = id;
    await api.setSubtitleTrack(id);
  }

  async function openSubtitle() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    await api.beginNativeDialog();
    try {
      const path = await open({ multiple: false, filters: [{ name: "Subtitles", extensions: ["srt", "ass", "ssa", "vtt"] }] });
      if (typeof path === "string") await api.addSubtitle(path);
    } finally {
      await api.endNativeDialog();
    }
  }

  function reloadStoryboard(_mediaId?: number) {
    window.dispatchEvent(new CustomEvent("slideup:storyboard-changed"));
  }

  async function setAspectRatio(ratio: string) {
    await api.setAspectRatio(ratio);
  }

  async function setAiEnhance(enabled: boolean, modelId?: string) {
    const targetModel = modelId ?? activeAiModel.value;
    if (modelId) activeAiModel.value = modelId;
    aiEnhance.value = await api.setAiEnhance(enabled, targetModel);
    return aiEnhance.value;
  }

  function toggleAiEnhance(modelId?: string) {
    return setAiEnhance(!aiEnhance.value, modelId);
  }

  async function setActiveAiModel(modelId: string) {
    activeAiModel.value = modelId;
    if (aiEnhance.value) {
      await setAiEnhance(true, modelId);
    }
  }

  /** Reflect which offline upscaler assets are installed. */
  async function refreshAiEnhanceAvailability() {
    try {
      const models = await api.listUpscalerModels();
      upscalerModels.value = models;
      aiEnhanceAvailable.value = models.some((model) => model.isDownloaded);
      realesrganAvailable.value = models.some(
        (model) => model.id === "realesr-general-x4v3" && model.isDownloaded,
      );
      // Auto-select first installed model if active model is not downloaded
      const currentInstalled = models.find((m) => m.id === activeAiModel.value && m.isDownloaded);
      if (!currentInstalled) {
        const firstInstalled = models.find((m) => m.isDownloaded);
        if (firstInstalled) activeAiModel.value = firstInstalled.id;
      }
      if (!aiEnhanceAvailable.value && aiEnhance.value) await setAiEnhance(false);
    } catch {
      aiEnhanceAvailable.value = false;
      realesrganAvailable.value = false;
    }
  }

  async function upscaleVideo(path: string) {
    return api.upscaleVideoWithRealEsrgan(path);
  }

  async function screenshot() {
    return api.screenshot();
  }

  async function frameStep() {
    await api.frameStep();
  }

  async function queueNext(media: MediaItem) {
    await api.queueNext(media);
    await refreshQueue();
  }

  async function queueAdd(media: MediaItem) {
    await api.queueAdd(media);
    await refreshQueue();
  }

  async function queueRemove(index: number) {
    await api.queueRemove(index);
    await refreshQueue();
  }

  async function queuePlayAt(index: number) {
    await api.queuePlayAt(index);
    await refreshQueue();
  }

  async function queueClear() {
    await api.queueClear();
    await refreshQueue();
  }

  function toggleQueue(open?: boolean) {
    queueOpen.value = open ?? !queueOpen.value;
  }

  return {
    state,
    queue,
    currentIndex,
    queueName,
    queueOpen,
    isPlaying,
    isVideo,
    hasTrack,
    isLive,
    progress,
    initialize,
    clearLocal,
    reload,
    dispose,
    refreshState,
    refreshQueue,
    play,
    playList,
    togglePause,
    next,
    previous,
    stop,
    seek,
    seekRelative,
    setVolume,
    setMuted,
    toggleMute,
    setSpeed,
    setShuffle,
    setRepeat,
    setFullscreen,
    isPip,
    togglePip,
    aiEnhance,
    aiEnhanceAvailable,
    realesrganAvailable,
    activeAiModel,
    activeAiModelName,
    upscalerModels,
    setAiEnhance,
    toggleAiEnhance,
    setActiveAiModel,
    refreshAiEnhanceAvailability,
    upscaleVideo,
    setVideoTrack,
    setAudioTrack,
    setSubtitleTrack,
    openSubtitle,
    reloadStoryboard,
    setAspectRatio,
    screenshot,
    frameStep,
    queueNext,
    queueAdd,
    queueRemove,
    queuePlayAt,
    queueClear,
    toggleQueue,
  };
});

export type { MediaTrack };
