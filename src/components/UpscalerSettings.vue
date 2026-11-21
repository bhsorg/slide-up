<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { Check, Download, LoaderCircle, Sparkles, Trash2, X } from "@lucide/vue";
import { api, events, onEvent } from "../services/tauri";
import { useToast } from "../composables/useToast";
import type { DownloadProgress, UpscalerModelStatus } from "../types/model";

const toast = useToast();

const models = ref<UpscalerModelStatus[]>([]);
const activeId = ref<string | null>(null);
const progress = ref<DownloadProgress | null>(null);
const busyId = ref<string | null>(null);
let unlisten: (() => void) | null = null;

const downloadedCount = computed(() => models.value.filter((model) => model.isDownloaded).length);

function formatSize(megabytes: number) {
  return megabytes < 1 ? `${Math.round(megabytes * 1024)} KB` : `${megabytes} MB`;
}

async function loadModels() {
  try {
    models.value = await api.listUpscalerModels();
  } catch (error) {
    toast.error(String(error), "Upscaler models");
  }
}

function handleProgress(payload: DownloadProgress) {
  progress.value = payload;
  if (payload.status === "completed") {
    activeId.value = null;
    void loadModels();
    toast.success("Upscaler model downloaded.");
  } else if (payload.status === "cancelled") {
    activeId.value = null;
    void loadModels();
    toast.info("Upscaler download cancelled.");
  } else if (payload.status === "error") {
    activeId.value = null;
    void loadModels();
  }
}

async function download(model: UpscalerModelStatus) {
  activeId.value = model.id;
  progress.value = { percentage: 0, downloadedMb: 0, totalMb: model.sizeMb, status: "downloading" };
  try {
    await api.downloadUpscalerModel(model.id);
  } catch (error) {
    activeId.value = null;
    progress.value = null;
    toast.error(String(error), "Upscaler download");
    void loadModels();
  }
}

async function cancel() {
  try {
    await api.cancelUpscalerDownload();
  } catch (error) {
    toast.error(String(error), "Upscaler download");
  }
}

async function remove(model: UpscalerModelStatus) {
  busyId.value = model.id;
  try {
    await api.deleteUpscalerModel(model.id);
    await loadModels();
    toast.success(`${model.name} model deleted.`);
  } catch (error) {
    toast.error(String(error), "Upscaler models");
  } finally {
    busyId.value = null;
  }
}

onMounted(async () => {
  unlisten = await onEvent<DownloadProgress>(events.upscalerDownloadProgress, handleProgress);
  await loadModels();
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <section class="su-panel upscaler-section">
    <div class="panel-body">
      <div class="model-head">
        <div class="model-icon"><Sparkles :size="20" /></div>
        <div class="model-intro">
          <div class="model-name">AI video upscalers</div>
          <div class="model-desc">Optional local models for sharpening and upscaling video frames</div>
        </div>
        <span class="status-pill">{{ downloadedCount }} / {{ models.length }} installed</span>
      </div>

      <div class="model-list">
        <article v-for="model in models" :key="model.id" class="model-row" :class="{ active: activeId === model.id }">
          <div class="model-info">
            <div class="model-title">
              <span>{{ model.name }}</span>
              <span class="size-tag">~{{ formatSize(model.sizeMb) }}</span>
              <span v-if="model.kind === 'shader'" class="status-pill is-kind">GLSL shader</span>
              <span v-if="model.isDownloaded" class="status-pill is-ready"><Check :size="12" /> Ready</span>
              <span v-else-if="!model.isAvailable" class="status-pill is-muted">Source URL needed</span>
            </div>
            <div class="model-detail">{{ model.description }}</div>
            <div class="model-meta">
              <span>{{ model.speed }}</span>
              <span class="dot">·</span>
              <span>{{ model.fileName }}</span>
              <template v-if="model.isDownloaded">
                <span class="dot">·</span>
                <span>{{ model.sizeOnDiskMb.toFixed(1) }} MB on disk</span>
              </template>
            </div>

            <div v-if="activeId === model.id" class="progress-block">
              <div class="progress-head">
                <span class="progress-label">Downloading…</span>
                <span class="progress-percent">{{ (progress?.percentage ?? 0).toFixed(1) }}%</span>
              </div>
              <div class="progress-track">
                <div class="progress-fill" :style="{ width: `${progress?.percentage ?? 0}%` }"></div>
              </div>
              <div class="progress-meta">
                <span>{{ (progress?.downloadedMb ?? 0).toFixed(2) }} MB / {{ (progress?.totalMb ?? model.sizeMb).toFixed(2) }} MB</span>
                <span>{{ progress?.status || "downloading" }}</span>
              </div>
            </div>
          </div>

          <div class="model-actions">
            <button
              v-if="activeId === model.id"
              type="button"
              class="btn-outline"
              @click="cancel"
            >
              <X :size="14" /> Cancel
            </button>
            <button
              v-else-if="model.isDownloaded"
              type="button"
              class="btn-outline danger"
              :disabled="busyId === model.id"
              @click="remove(model)"
            >
              <LoaderCircle v-if="busyId === model.id" :size="14" class="spin" />
              <Trash2 v-else :size="14" /> Delete
            </button>
            <button
              v-else
              type="button"
              class="btn-primary"
              :disabled="!model.isAvailable || activeId !== null"
              :title="model.isAvailable ? 'Download model' : 'No verified download URL configured yet'"
              @click="download(model)"
            >
              <Download :size="14" /> Download
            </button>
          </div>
        </article>
      </div>

      <p class="footnote">
        ONNX models are stored in <code>models/upscalers</code> and GLSL shaders in
        <code>shaders</code> under your application data folder. Only entries with a verified
        source URL can be downloaded; the others are listed as a catalog until a source is
        confirmed. Once the FSRCNNX shader is installed, the video player shows an
        <strong>AI Enhance</strong> button to apply it at runtime; ONNX upscaler models are
        downloaded for future playback integration and are not applied yet.
      </p>
    </div>
  </section>
</template>

<style scoped>
.upscaler-section { overflow: hidden; }
.panel-body { padding: 1.1rem 1.25rem 0.9rem; }

.model-head { display: flex; align-items: center; gap: 0.85rem; margin-bottom: 1rem; }
.model-icon {
  width: 38px; height: 38px; border-radius: 11px;
  display: grid; place-items: center;
  background: var(--accent-gradient-soft); color: var(--text-primary);
}
.model-intro { flex: 1; min-width: 0; }
.model-name { font-size: 0.95rem; font-weight: 600; color: var(--text-primary); }
.model-desc { font-size: 0.76rem; color: var(--text-secondary); margin-top: 2px; }
.status-pill {
  display: inline-flex; align-items: center; gap: 4px;
  font-size: 0.68rem; font-weight: 600;
  padding: 3px 9px; border-radius: 999px;
  background: var(--bg-elevated); color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  white-space: nowrap;
}
.status-pill.is-ready {
  color: #7ee2a8; background: rgba(46, 204, 113, 0.12);
  border-color: rgba(46, 204, 113, 0.28);
}
.status-pill.is-muted { color: var(--text-muted); }
.status-pill.is-kind { color: var(--accent-blue); border-color: rgba(40, 184, 255, 0.3); }

.model-list { display: flex; flex-direction: column; gap: 0.6rem; }
.model-row {
  display: flex; align-items: center; justify-content: space-between; gap: 1rem;
  padding: 0.85rem 0.95rem;
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  background: var(--bg-elevated);
  transition: border-color var(--t-fast), background var(--t-fast);
}
.model-row.active { border-color: var(--accent-purple); }
.model-info { flex: 1; min-width: 0; }
.model-title { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; font-weight: 600; font-size: 0.85rem; color: var(--text-primary); }
.size-tag { font-size: 0.68rem; color: var(--text-muted); font-weight: 500; }
.model-detail { font-size: 0.76rem; color: var(--text-secondary); margin-top: 4px; }
.model-meta { display: flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; margin-top: 5px; font-size: 0.7rem; color: var(--text-muted); }
.dot { opacity: 0.6; }
.model-actions { flex-shrink: 0; }

.progress-block { margin-top: 0.7rem; max-width: 420px; }
.progress-head { display: flex; justify-content: space-between; font-size: 0.72rem; color: var(--text-secondary); margin-bottom: 5px; }
.progress-label { font-weight: 600; }
.progress-percent { color: var(--text-primary); font-variant-numeric: tabular-nums; }
.progress-track { height: 6px; border-radius: 999px; background: var(--bg-active); overflow: hidden; }
.progress-fill {
  height: 100%; border-radius: 999px; background: var(--accent-gradient);
  transition: width var(--t-fast) linear;
}
.progress-meta { display: flex; justify-content: space-between; margin-top: 5px; font-size: 0.68rem; color: var(--text-muted); }

.btn-primary, .btn-outline {
  display: inline-flex; align-items: center; justify-content: center; gap: 6px;
  min-height: 32px; padding: 0 0.8rem;
  border-radius: 9px; font-size: 0.76rem; font-weight: 600; cursor: pointer;
  transition: filter var(--t-fast), background var(--t-fast), border-color var(--t-fast);
}
.btn-primary { border: 1px solid transparent; background: var(--accent-gradient); color: #fff; }
.btn-primary:hover:not(:disabled) { filter: brightness(1.12); }
.btn-outline { border: 1px solid var(--border-strong); background: transparent; color: var(--text-primary); }
.btn-outline:hover:not(:disabled) { background: var(--bg-active); }
.btn-outline.danger { color: #ff8b9c; border-color: rgba(255, 99, 132, 0.35); }
.btn-primary:disabled, .btn-outline:disabled { opacity: 0.45; cursor: not-allowed; }
.spin { animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.footnote { font-size: 0.72rem; color: var(--text-muted); line-height: 1.55; margin: 0.9rem 0 0; }

@media (max-width: 900px) {
  .model-row { flex-direction: column; align-items: stretch; }
  .model-actions { align-self: flex-end; }
}
</style>
