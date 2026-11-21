<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  Check,
  Cpu,
  Download,
  HardDrive,
  LoaderCircle,
  RefreshCw,
  Sparkles,
  Trash2,
  Zap,
} from "@lucide/vue";
import { api, events, onEvent } from "../services/tauri";
import { useToast } from "../composables/useToast";
import { useSettingsStore } from "../stores/settings";
import type { DownloadProgress, ModelStatus } from "../types/model";

const toast = useToast();
const settings = useSettingsStore();

const status = ref<ModelStatus | null>(null);
const progress = ref<DownloadProgress | null>(null);
const started = ref(false);
const busy = ref(false);
let unlisten: (() => void) | null = null;

const downloading = computed(() => started.value);
const isGpu = computed(() => /gpu/i.test(status.value?.activeProvider ?? ""));
const downloaded = computed(() => status.value?.isDownloaded === true && !downloading.value);

async function loadStatus() {
  try {
    status.value = await api.checkModelStatus();
  } catch (error) {
    toast.error(String(error), "Caption model");
  }
}

function handleProgress(payload: DownloadProgress) {
  progress.value = payload;
  if (payload.status === "completed") {
    started.value = false;
    void loadStatus();
    toast.success("Caption model is ready.");
  } else if (payload.status === "cancelled") {
    started.value = false;
    void loadStatus();
    toast.info("Model download cancelled.");
  } else if (payload.status === "error") {
    started.value = false;
    void loadStatus();
  }
}

async function download() {
  started.value = true;
  progress.value = { percentage: 0, downloadedMb: 0, totalMb: 0, status: "downloading" };
  try {
    await api.downloadCaptionModel();
  } catch (error) {
    started.value = false;
    toast.error(String(error), "Model download");
    void loadStatus();
  }
}

async function cancel() {
  try {
    await api.cancelModelDownload();
  } catch (error) {
    toast.error(String(error), "Model download");
  }
}

async function loadEngine() {
  busy.value = true;
  try {
    await api.loadCaptionEngine();
    await loadStatus();
    toast.success("Caption engine loaded.");
  } catch (error) {
    toast.error(String(error), "Caption engine");
  } finally {
    busy.value = false;
  }
}

async function remove() {
  busy.value = true;
  try {
    await api.deleteCaptionModel();
    started.value = false;
    progress.value = null;
    await loadStatus();
    toast.success("Caption model deleted.");
  } catch (error) {
    toast.error(String(error), "Caption model");
  } finally {
    busy.value = false;
  }
}

async function toggleCaptions(event: Event) {
  const enabled = (event.target as HTMLInputElement).checked;
  try {
    await settings.patch({ liveCaptionsEnabled: enabled });
  } catch (error) {
    toast.error(String(error), "Live captions");
  }
}

onMounted(async () => {
  unlisten = await onEvent<DownloadProgress>(events.modelDownloadProgress, handleProgress);
  await loadStatus();
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <section class="su-panel caption-section">
    <div class="panel-body">
      <div class="model-head">
        <div class="model-icon"><Sparkles :size="20" /></div>
        <div class="model-intro">
          <div class="model-name">SenseVoice Multilingual</div>
          <div class="model-desc">Hindi, Hinglish &amp; English · Offline real-time captions · ~230 MB</div>
        </div>
        <span v-if="downloaded" class="status-pill is-ready"><Check :size="12" /> Ready</span>
      </div>

      <!-- State 1: not downloaded -->
      <div v-if="!downloading && !downloaded" class="model-state">
        <p class="footnote">
          The model, tokenizer and voice-activity detector are stored locally under your
          application data folder. Everything runs offline — no audio or video is uploaded.
        </p>
        <div class="model-actions">
          <button type="button" class="btn-primary" @click="download">
            <Download :size="14" /> Download Model
          </button>
        </div>
      </div>

      <!-- State 2: downloading -->
      <div v-else-if="downloading" class="model-state">
        <div class="progress-head">
          <span class="progress-label">Downloading model…</span>
          <span class="progress-percent">{{ (progress?.percentage ?? 0).toFixed(1) }}%</span>
        </div>
        <div class="progress-track">
          <div class="progress-fill" :style="{ width: `${progress?.percentage ?? 0}%` }"></div>
        </div>
        <div class="progress-meta">
          <span>{{ (progress?.downloadedMb ?? 0).toFixed(1) }} MB / {{ (progress?.totalMb ?? 0).toFixed(1) }} MB</span>
          <span>{{ progress?.status || "downloading" }}</span>
        </div>
        <div class="model-actions">
          <button type="button" class="btn-outline" @click="cancel">Cancel</button>
        </div>
      </div>

      <!-- State 3: downloaded and ready -->
      <div v-else class="model-state">
        <div class="ready-row">
          <div class="hardware-badge" :class="isGpu ? 'is-gpu' : 'is-cpu'">
            <component :is="isGpu ? Zap : Cpu" :size="13" />
            {{ status?.activeProvider || "Not loaded" }}
          </div>
          <span class="model-size"><HardDrive :size="13" /> {{ status?.modelSizeMb ?? 0 }} MB on disk</span>
        </div>

        <div v-if="status?.isDownloaded && !status?.isLoaded" class="load-hint">
          <span>The engine is not running yet.</span>
          <button type="button" class="link-btn" :disabled="busy" @click="loadEngine">
            <RefreshCw v-if="busy" :size="12" class="spin" /> Load engine
          </button>
        </div>

        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">Live captions</div>
            <div class="setting-hint">Enable on-screen captions during video playback.</div>
          </div>
          <div class="setting-control">
            <input
              class="toggle"
              type="checkbox"
              :checked="settings.settings.liveCaptionsEnabled"
              :disabled="!status?.isLoaded"
              @change="toggleCaptions"
            />
          </div>
        </div>

        <p class="footnote">
          The engine provides voice-activity detection and transcription, but live audio
          capture and the on-screen overlay are not wired to playback yet. The hardware
          badge reflects the bundled ONNX Runtime build — the stock DLL is CPU-only.
        </p>
        <div class="model-actions">
          <button type="button" class="btn-outline danger" :disabled="busy" @click="remove">
            <Trash2 :size="14" /> Delete Model
          </button>
        </div>
      </div>

      <div v-if="busy && downloading" class="busy-line"><LoaderCircle :size="13" class="spin" /> Working…</div>
    </div>
  </section>
</template>

<style scoped>
.caption-section { overflow: hidden; }
.panel-body { padding: 1.1rem 1.25rem 0.9rem; }

.model-head { display: flex; align-items: center; gap: 0.85rem; }
.model-icon { display: flex; height: 2.6rem; width: 2.6rem; flex-shrink: 0; align-items: center; justify-content: center; border-radius: 0.8rem; background: var(--accent-gradient-soft); color: var(--accent-purple-soft); }
.model-intro { min-width: 0; flex: 1; }
.model-name { font-size: 0.88rem; font-weight: 700; color: var(--text-primary); }
.model-desc { margin-top: 0.15rem; font-size: 0.7rem; color: var(--text-muted); }
.status-pill { display: inline-flex; height: 1.7rem; align-items: center; gap: 0.3rem; border-radius: 999px; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.7rem; font-size: 0.68rem; color: var(--text-muted); }
.status-pill.is-ready { border-color: rgba(74, 222, 128, 0.4); background: rgba(74, 222, 128, 0.12); color: #86efac; }

.model-state { margin-top: 1.1rem; }
.model-actions { display: flex; gap: 0.6rem; margin-top: 0.9rem; }
.btn-primary { display: flex; height: 2.2rem; align-items: center; gap: 0.4rem; border-radius: 0.6rem; background: var(--accent-gradient); padding: 0 0.9rem; font-size: 0.72rem; font-weight: 600; color: #fff; transition: filter var(--t-fast); }
.btn-primary:hover { filter: brightness(1.12); }
.btn-outline { display: flex; height: 2.2rem; align-items: center; gap: 0.4rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.85rem; font-size: 0.72rem; color: var(--text-secondary); transition: color var(--t-fast), border-color var(--t-fast); }
.btn-outline:hover { color: var(--text-primary); border-color: var(--border-strong); }
.btn-outline.danger:hover { color: #ff8f8f; border-color: rgba(255, 143, 143, 0.5); }
.btn-outline:disabled { opacity: 0.5; cursor: not-allowed; }

.progress-head, .progress-meta { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
.progress-label { font-size: 0.75rem; font-weight: 600; color: var(--text-primary); }
.progress-percent { font-size: 0.75rem; font-weight: 700; color: var(--accent-purple-soft); font-variant-numeric: tabular-nums; }
.progress-track { height: 0.5rem; margin-top: 0.6rem; overflow: hidden; border-radius: 999px; background: var(--bg-active); }
.progress-fill { height: 100%; border-radius: 999px; background: var(--accent-gradient); transition: width 200ms linear; }
.progress-meta { margin-top: 0.5rem; font-size: 0.68rem; color: var(--text-muted); font-variant-numeric: tabular-nums; }

.ready-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
.hardware-badge { display: inline-flex; height: 1.9rem; align-items: center; gap: 0.4rem; border-radius: 0.5rem; padding: 0 0.7rem; font-size: 0.7rem; font-weight: 600; }
.hardware-badge.is-gpu { background: rgba(40, 184, 255, 0.12); color: #7dd3ff; }
.hardware-badge.is-cpu { background: var(--bg-active); color: var(--text-secondary); }
.model-size { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.68rem; color: var(--text-muted); }
.load-hint { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin-top: 0.8rem; border-radius: 0.5rem; background: var(--bg-elevated); padding: 0.55rem 0.75rem; font-size: 0.7rem; color: var(--text-secondary); }
.link-btn { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.7rem; font-weight: 600; color: var(--accent-purple-soft); }
.link-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.setting-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 1.5rem; margin-top: 1rem; padding: 0.8rem 0; border-top: 1px solid var(--border-subtle); border-bottom: 1px solid var(--border-subtle); }
.setting-info { min-width: 0; }
.setting-name { font-size: 0.82rem; font-weight: 600; color: var(--text-primary); }
.setting-hint { margin-top: 0.15rem; font-size: 0.7rem; line-height: 1.45; color: var(--text-muted); }
.setting-control { display: flex; justify-content: flex-end; }
.footnote { margin: 0; font-size: 0.68rem; line-height: 1.45; color: var(--text-muted); }
.footnote + .model-actions { margin-top: 0.9rem; }
.busy-line { display: inline-flex; align-items: center; gap: 0.4rem; margin-top: 0.6rem; font-size: 0.68rem; color: var(--text-muted); }
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
