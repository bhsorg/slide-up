<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Settings as SettingsIcon, FolderOpen, Keyboard, RotateCcw, Monitor, SlidersHorizontal, Subtitles, Palette, Volume2, Info, Link2, Plus, Trash2, Check, ExternalLink, Captions, Expand, Film } from "@lucide/vue";
import CaptionSettings from "../components/CaptionSettings.vue";
import UpscalerSettings from "../components/UpscalerSettings.vue";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "../composables/useToast";
import { api } from "../services/tauri";
import type { AudioDevice, Shortcuts, FileAssociations, ViewId } from "../types/settings";

const emit = defineEmits<{ (e: "navigate", view: ViewId): void }>();

const settings = useSettingsStore();
const toast = useToast();
const capturing = ref<keyof Shortcuts | null>(null);
const audioDevices = ref<AudioDevice[]>([]);
const audioOutputs = [
  { value: "auto", label: "Automatic (WASAPI)" },
  { value: "wasapi", label: "WASAPI (Windows Default)" },
  { value: "openal", label: "OpenAL" },
  { value: "sdl", label: "SDL" },
  { value: "null", label: "No audio output" },
];

// Vertical settings navigation
const tabs = [
  { id: "appearance", label: "Appearance", icon: Palette, description: "Choose how Slide Up looks." },
  { id: "library", label: "Library scanning", icon: FolderOpen, description: "Control which files are imported into your library." },
  { id: "startup", label: "Startup", icon: Monitor, description: "Configure what happens when Slide Up launches." },
  { id: "playback", label: "Playback", icon: SlidersHorizontal, description: "Set default volume, speed and playback behaviour." },
  { id: "audio", label: "Audio output", icon: Volume2, description: "Choose the audio backend and output device." },
  { id: "subtitles", label: "Subtitles", icon: Subtitles, description: "Fine-tune subtitle appearance and timing." },
  { id: "video", label: "Video & filters", icon: Film, description: "Adjust brightness, contrast, saturation and gamma." },
  { id: "captions", label: "Live captions", icon: Captions, description: "Manage the offline SenseVoice caption model and hardware acceleration." },
  { id: "upscalers", label: "AI upscaling", icon: Expand, description: "Download local SPAN, Real-ESRGAN and ESPCN upscaler models." },
  { id: "associations", label: "File associations", icon: Link2, description: "Choose which file types open with Slide Up." },
  { id: "shortcuts", label: "Keyboard shortcuts", icon: Keyboard, description: "Customize shortcuts for playback and navigation." },
  { id: "about", label: "About", icon: Info, description: "Information about this build." },
] as const;

type TabId = (typeof tabs)[number]["id"];
const activeTab = ref<TabId>(
  tabs.some((t) => t.id === settings.activeTab) ? (settings.activeTab as TabId) : "appearance",
);

watch(
  () => settings.activeTab,
  (newTab) => {
    if (newTab && tabs.some((t) => t.id === newTab)) {
      activeTab.value = newTab as TabId;
    }
  },
);
const activeTabMeta = computed(() => tabs.find((tab) => tab.id === activeTab.value)!);

const associations = ref<FileAssociations | null>(null);
const assocBusy = ref(false);
const customExt = ref("");
const customKind = ref<"audio" | "video">("audio");
const builtInCount = computed(
  () => (associations.value?.audio.length ?? 0) + (associations.value?.video.length ?? 0),
);
const registeredCount = computed(() =>
  [...(associations.value?.audio ?? []), ...(associations.value?.video ?? [])].filter(
    (item) => item.registered,
  ).length,
);

async function loadAssociations() {
  try {
    associations.value = await api.getFileAssociations();
  } catch (error) {
    toast.error(String(error), "Could not load file associations");
  }
}

watch(activeTab, (tab) => {
  if (tab === "associations") void loadAssociations();
});

async function runAssociationAction(action: () => Promise<FileAssociations>, message: string) {
  assocBusy.value = true;
  try {
    associations.value = await action();
    toast.success(message);
  } catch (error) {
    toast.error(String(error), "File associations");
  } finally {
    assocBusy.value = false;
  }
}

function registerAll() {
  return runAssociationAction(
    () => api.registerFileAssociations(),
    "File associations registered.",
  );
}

function unregisterAll() {
  return runAssociationAction(
    () => api.unregisterFileAssociations(),
    "File associations removed.",
  );
}

async function addCustom() {
  const ext = customExt.value.trim();
  if (!ext) return;
  await runAssociationAction(
    () => api.addCustomExtension(ext, customKind.value),
    `Added .${ext.replace(/^\./, "")}.`,
  );
  customExt.value = "";
}

function removeCustom(ext: string) {
  return runAssociationAction(
    () => api.removeCustomExtension(ext),
    `Removed .${ext}.`,
  );
}

async function openDefaultAppsSettings() {
  try {
    await api.openDefaultAppsSettings();
  } catch (error) {
    toast.error(String(error), "Default Apps");
  }
}

async function loadAudioDevices() {
  try {
    audioDevices.value = await api.listAudioDevices();
  } catch {
    audioDevices.value = [];
  }
}

onMounted(() => {
  void loadAudioDevices();
  void loadAssociations();
});

const shortcutLabels: Array<{ key: keyof Shortcuts; label: string }> = [
  { key: "playPause", label: "Play / Pause" },
  { key: "seekBack", label: "Seek backward" },
  { key: "seekForward", label: "Seek forward" },
  { key: "volumeUp", label: "Volume up" },
  { key: "volumeDown", label: "Volume down" },
  { key: "fullscreen", label: "Fullscreen" },
  { key: "mute", label: "Mute / Unmute" },
  { key: "next", label: "Next track" },
  { key: "previous", label: "Previous track" },
  { key: "exitFullscreen", label: "Exit fullscreen" },
  { key: "openFile", label: "Open file" },
  { key: "openFolder", label: "Open folder" },
];

const folderLabel = computed(() => settings.settings.defaultMediaFolder ?? "Not set");

async function save(partial: Partial<typeof settings.settings>) {
  try {
    await settings.patch(partial);
  } catch (error) {
    toast.error(String(error), "Could not save settings");
  }
}

type FilterKey = "videoBrightness" | "videoContrast" | "videoSaturation" | "videoGamma";

async function changeFilter(key: FilterKey, value: number) {
  const property = key.slice(5).toLowerCase();
  void api.setVideoFilter(property, value).catch(() => {});
  await save({ [key]: value } as Partial<typeof settings.settings>);
}

async function applyAutoFilters() {
  try {
    const filters = await api.autoVideoFilters();
    await save({
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

async function chooseFolder() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const folder = await open({ directory: true, multiple: false });
    if (typeof folder === "string") await save({ defaultMediaFolder: folder });
  } catch (error) {
    toast.error(String(error), "Could not select folder");
  }
}

function comboFromEvent(event: KeyboardEvent) {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.shiftKey) parts.push("Shift");
  if (event.altKey) parts.push("Alt");
  if (event.metaKey) parts.push("Meta");
  const raw = event.key;
  const key = raw === " " ? "Space" : raw.length === 1 && parts.length ? raw.toUpperCase() : raw.length === 1 ? raw.toLowerCase() : raw;
  parts.push(key);
  return parts.join("+");
}

async function capture(event: KeyboardEvent, key: keyof Shortcuts) {
  event.preventDefault();
  if (event.key === "Escape") {
    capturing.value = null;
    return;
  }
  const combo = comboFromEvent(event);
  capturing.value = null;
  try {
    await settings.setShortcut(key, combo);
  } catch (error) {
    toast.error(String(error), "Could not save shortcut");
  }
}

async function resetAll() {
  try {
    await settings.resetAll();
    toast.success("Settings restored to defaults.");
  } catch (error) {
    toast.error(String(error), "Could not reset settings");
  }
}
</script>

<template>
  <div class="flex h-full flex-col px-5 py-4">
    <header class="page-header">
      <div class="page-heading">
        <span class="page-icon"><SettingsIcon :size="22" /></span>
        <div class="min-w-0">
          <h1>Settings</h1>
          <p>Tailor Slide Up to the way you like it</p>
        </div>
      </div>
      <button type="button" class="btn-outline" @click="resetAll"><RotateCcw :size="14" /> Reset all</button>
    </header>

    <div class="settings-layout">
      <!-- Left vertical tab rail -->
      <nav class="tabs-panel su-panel" aria-label="Settings sections">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          class="tab-btn"
          :class="activeTab === tab.id ? 'tab-active' : ''"
          @click="activeTab = tab.id"
        >
          <component :is="tab.icon" :size="16" class="tab-icon" />
          <span>{{ tab.label }}</span>
        </button>
      </nav>

      <!-- Active section content (full width of the remaining area) -->
      <div class="content-panel">
        <div class="content-head">
          <div class="min-w-0">
            <h2>{{ activeTabMeta.label }}</h2>
            <p>{{ activeTabMeta.description }}</p>
          </div>
          <button v-if="activeTab === 'shortcuts'" type="button" class="link-btn" @click="settings.resetShortcuts()">Reset defaults</button>
        </div>

        <!-- Appearance -->
        <section v-if="activeTab === 'appearance'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Theme</div>
                <div class="setting-hint">Dark is easy on the eyes; light is great in daylight.</div>
              </div>
              <div class="setting-control">
                <button v-for="option in ['dark', 'light'] as const" :key="option" type="button" class="seg-btn" :class="settings.theme === option ? 'seg-active' : ''" @click="settings.setTheme(option)">
                  {{ option === 'dark' ? 'Dark' : 'Light' }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- Library scanning -->
        <section v-else-if="activeTab === 'library'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Default media folder</div>
                <div class="setting-hint">{{ folderLabel }}</div>
              </div>
              <div class="setting-control">
                <button type="button" class="btn-outline" @click="chooseFolder">Choose folder</button>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Scan subfolders</div>
                <div class="setting-hint">Walk nested folders when importing a library folder.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.scanRecursive" @change="save({ scanRecursive: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Include hidden files</div>
                <div class="setting-hint">Import files whose names start with a dot.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.scanHidden" @change="save({ scanHidden: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Scan audio files</div>
                <div class="setting-hint">Import MP3, WAV, FLAC, AAC, OGG and M4A.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.scanAudio" @change="save({ scanAudio: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Scan video files</div>
                <div class="setting-hint">Import MP4, MKV, AVI, MOV and WebM.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.scanVideo" @change="save({ scanVideo: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Generate thumbnails</div>
                <div class="setting-hint">Capture cover art and cached seek-preview frames. Turn off for faster scans.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.generateThumbnails" @change="save({ generateThumbnails: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Remove missing files from library</div>
                <div class="setting-hint">Prune entries when a scan no longer finds them on disk.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.removeMissingOnScan" @change="save({ removeMissingOnScan: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
          </div>
        </section>

        <!-- Startup -->
        <section v-else-if="activeTab === 'startup'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Scan library on startup</div>
                <div class="setting-hint">Check the default folder for new media when the app opens.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.autoScanOnStartup" @change="save({ autoScanOnStartup: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Start minimized</div>
                <div class="setting-hint">Launch Slide Up in the system tray.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.startMinimized" @change="save({ startMinimized: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Close button minimizes to tray</div>
                <div class="setting-hint">Keep playback running when the window is closed. Click the tray icon to restore.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.minimizeToTray" @change="save({ minimizeToTray: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Taskbar media controls (Windows)</div>
                <div class="setting-hint">Show playback buttons (Play, Pause, Next, Prev, Exit) on taskbar thumbnail preview and keep music playing in background on close.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.taskbarControls" @change="save({ taskbarControls: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Desktop player widget</div>
                <div class="setting-hint">Show the compact always-on-top player when Slide Up starts.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.playerWidget" @change="save({ playerWidget: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div v-if="settings.settings.playerWidget" class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Minimum opacity — {{ settings.settings.playerWidgetMinOpacity }}%</div>
                <div class="setting-hint">Used when the pointer is away from the widget.</div>
              </div>
              <div class="setting-control">
                <input
                  type="range"
                  min="0"
                  :max="settings.settings.playerWidgetMaxOpacity"
                  step="1"
                  class="su-range w-48"
                  :value="settings.settings.playerWidgetMinOpacity"
                  :style="{ '--fill': `${settings.settings.playerWidgetMinOpacity}%` }"
                  aria-label="Minimum player widget opacity"
                  @change="save({ playerWidgetMinOpacity: Number(($event.target as HTMLInputElement).value) })"
                />
              </div>
            </div>
            <div v-if="settings.settings.playerWidget" class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Maximum opacity — {{ settings.settings.playerWidgetMaxOpacity }}%</div>
                <div class="setting-hint">Used while the pointer is over the widget.</div>
              </div>
              <div class="setting-control">
                <input
                  type="range"
                  :min="settings.settings.playerWidgetMinOpacity"
                  max="100"
                  step="1"
                  class="su-range w-48"
                  :value="settings.settings.playerWidgetMaxOpacity"
                  :style="{ '--fill': `${settings.settings.playerWidgetMaxOpacity}%` }"
                  aria-label="Maximum player widget opacity"
                  @change="save({ playerWidgetMaxOpacity: Number(($event.target as HTMLInputElement).value) })"
                />
              </div>
            </div>
          </div>
        </section>

        <!-- Playback -->
        <section v-else-if="activeTab === 'playback'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Default volume — {{ Math.round(settings.settings.volume * 100) }}%</div>
                <div class="setting-hint">Applied when the app starts.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="0" max="1" step="0.01" class="su-range w-48" :value="settings.settings.volume" :style="{ '--fill': `${settings.settings.volume * 100}%` }" @change="save({ volume: Number(($event.target as HTMLInputElement).value) })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Default playback speed — {{ settings.settings.playbackSpeed }}×</div>
                <div class="setting-hint">Applied to newly loaded media.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="0.25" max="4" step="0.25" class="su-range w-48" :value="settings.settings.playbackSpeed" :style="{ '--fill': `${(settings.settings.playbackSpeed / 4) * 100}%` }" @change="save({ playbackSpeed: Number(($event.target as HTMLInputElement).value) })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Shuffle by default</div>
                <div class="setting-hint">Start new queues in random order.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.shuffle" @change="save({ shuffle: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Repeat mode</div>
                <div class="setting-hint">Off, repeat the whole queue, or repeat one item.</div>
              </div>
              <div class="setting-control">
                <select class="control-select" :value="settings.settings.repeat" @change="save({ repeat: ($event.target as HTMLSelectElement).value as 'off' | 'all' | 'one' })">
                  <option value="off">Off</option><option value="all">Repeat all</option><option value="one">Repeat one</option>
                </select>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Hardware acceleration</div>
                <div class="setting-hint">Use GPU decoding when available.</div>
              </div>
              <div class="setting-control">
                <input type="checkbox" class="toggle" :checked="settings.settings.hardwareAcceleration" @change="save({ hardwareAcceleration: ($event.target as HTMLInputElement).checked })" />
              </div>
            </div>
          </div>
        </section>

        <!-- Audio output -->
        <section v-else-if="activeTab === 'audio'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Output API</div>
                <div class="setting-hint">Force a specific Windows audio backend, or let mpv choose.</div>
              </div>
              <div class="setting-control">
                <select class="control-select" :value="settings.settings.audioOutput" @change="save({ audioOutput: ($event.target as HTMLSelectElement).value })">
                  <option v-for="output in audioOutputs" :key="output.value" :value="output.value">{{ output.label }}</option>
                </select>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Output device</div>
                <div class="setting-hint">{{ audioDevices.length ? 'Choose where audio is sent.' : 'No output devices detected by the engine.' }}</div>
              </div>
              <div class="setting-control">
                <select class="control-select" :value="settings.settings.audioDevice" @change="save({ audioDevice: ($event.target as HTMLSelectElement).value })">
                  <option value="auto">System default</option>
                  <option v-for="device in audioDevices" :key="device.name" :value="device.name">{{ device.description }}</option>
                </select>
              </div>
            </div>
            <p class="footnote">Changes apply to newly loaded media. If no device is active, Windows audio must be enabled first.</p>
          </div>
        </section>

        <!-- Subtitles -->
        <section v-else-if="activeTab === 'subtitles'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Font family</div>
                <div class="setting-hint">Used for SRT, ASS, SSA and VTT subtitles.</div>
              </div>
              <div class="setting-control">
                <input class="text-input" :value="settings.settings.subtitleFont" @change="save({ subtitleFont: ($event.target as HTMLInputElement).value })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Font size — {{ settings.settings.subtitleSize }}px</div>
                <div class="setting-hint">Subtitle text size.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="12" max="48" step="1" class="su-range w-48" :value="settings.settings.subtitleSize" :style="{ '--fill': `${((settings.settings.subtitleSize - 12) / 36) * 100}%` }" @change="save({ subtitleSize: Number(($event.target as HTMLInputElement).value) })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Text color</div>
                <div class="setting-hint">Applied to SRT, VTT and plain-text subtitles.</div>
              </div>
              <div class="setting-control">
                <input type="color" class="color-input" :value="settings.settings.subtitleColor" @change="save({ subtitleColor: ($event.target as HTMLInputElement).value })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Outline — {{ settings.settings.subtitleBorderSize }}px</div>
                <div class="setting-hint">Improves readability on bright video.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="0" max="8" step="1" class="su-range w-48" :value="settings.settings.subtitleBorderSize" :style="{ '--fill': `${(settings.settings.subtitleBorderSize / 8) * 100}%` }" @change="save({ subtitleBorderSize: Number(($event.target as HTMLInputElement).value) })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Bottom margin — {{ settings.settings.subtitleMargin }}px</div>
                <div class="setting-hint">Distance from the bottom of the video.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="0" max="80" step="2" class="su-range w-48" :value="settings.settings.subtitleMargin" :style="{ '--fill': `${(settings.settings.subtitleMargin / 80) * 100}%` }" @change="save({ subtitleMargin: Number(($event.target as HTMLInputElement).value) })" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Default sync offset</div>
                <div class="setting-hint">Positive values delay subtitles; negative values show them early.</div>
              </div>
              <div class="setting-control">
                <input type="number" min="-30000" max="30000" step="100" class="text-input w-28" :value="settings.settings.subtitleDelayMs" @change="save({ subtitleDelayMs: Number(($event.target as HTMLInputElement).value) })" />
                <span class="unit">ms</span>
              </div>
            </div>
          </div>
        </section>

        <!-- Video & filters -->
        <section v-else-if="activeTab === 'video'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Auto optimize</div>
                <div class="setting-hint">Analyze the current frame and apply optimal filter values.</div>
              </div>
              <div class="setting-control">
                <button type="button" class="btn-outline" @click="applyAutoFilters">Auto</button>
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Brightness — {{ settings.settings.videoBrightness }}</div>
                <div class="setting-hint">Change the overall lightness of the picture.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="-100" max="100" step="1" class="su-range w-48" :value="settings.settings.videoBrightness" :style="{ '--fill': `${((settings.settings.videoBrightness + 100) / 200) * 100}%` }" @change="changeFilter('videoBrightness', Number(($event.target as HTMLInputElement).value))" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Contrast — {{ settings.settings.videoContrast }}</div>
                <div class="setting-hint">Difference between light and dark areas.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="-100" max="100" step="1" class="su-range w-48" :value="settings.settings.videoContrast" :style="{ '--fill': `${((settings.settings.videoContrast + 100) / 200) * 100}%` }" @change="changeFilter('videoContrast', Number(($event.target as HTMLInputElement).value))" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Saturation — {{ settings.settings.videoSaturation }}</div>
                <div class="setting-hint">Intensity of colors.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="-100" max="100" step="1" class="su-range w-48" :value="settings.settings.videoSaturation" :style="{ '--fill': `${((settings.settings.videoSaturation + 100) / 200) * 100}%` }" @change="changeFilter('videoSaturation', Number(($event.target as HTMLInputElement).value))" />
              </div>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Gamma — {{ settings.settings.videoGamma }}</div>
                <div class="setting-hint">Brightness of mid-tone colors.</div>
              </div>
              <div class="setting-control">
                <input type="range" min="-100" max="100" step="1" class="su-range w-48" :value="settings.settings.videoGamma" :style="{ '--fill': `${((settings.settings.videoGamma + 100) / 200) * 100}%` }" @change="changeFilter('videoGamma', Number(($event.target as HTMLInputElement).value))" />
              </div>
            </div>
          </div>
        </section>

        <!-- File associations -->
        <section v-else-if="activeTab === 'associations'" class="su-panel settings-section">
          <div class="panel-body assoc-body">
            <div class="assoc-status">
              <div>
                <div class="setting-name">
                  {{ associations?.registered ? "Slide Up is registered" : "Not registered" }}
                </div>
                <div class="setting-hint">
                  {{ registeredCount }} of {{ builtInCount }} built-in formats are currently
                  associated with Slide Up on this PC.
                </div>
              </div>
              <span class="status-pill" :class="associations?.registered ? 'is-on' : ''">
                <Check v-if="associations?.registered" :size="12" />
                {{ associations?.registered ? "Active" : "Inactive" }}
              </span>
            </div>

            <div class="assoc-actions">
              <button type="button" class="btn-primary" :disabled="assocBusy" @click="registerAll">
                <Link2 :size="13" /> Register all formats
              </button>
              <button
                type="button"
                class="btn-outline"
                :disabled="assocBusy || !associations?.registered"
                @click="unregisterAll"
              >
                Unregister all
              </button>
              <button
                type="button"
                class="btn-outline"
                :disabled="assocBusy || !associations?.registered"
                @click="openDefaultAppsSettings"
              >
                <ExternalLink :size="13" /> Choose default apps
              </button>
            </div>

            <p class="footnote">
              Windows protects the default-app choice. After registering, open
              <strong>Choose default apps</strong> and confirm Slide Up for the formats you want
              double-click to open. Explorer's <em>Open with</em> list should also show Slide Up.
            </p>

            <div class="assoc-group">
              <div class="assoc-group-title">
                Built-in audio formats <span>{{ associations?.audio.length ?? 0 }} formats</span>
              </div>
              <div class="ext-grid">
                <span
                  v-for="item in associations?.audio ?? []"
                  :key="item.ext"
                  class="ext-chip"
                  :class="item.registered ? 'is-on' : ''"
                >.{{ item.ext }}</span>
              </div>
            </div>

            <div class="assoc-group">
              <div class="assoc-group-title">
                Built-in video formats <span>{{ associations?.video.length ?? 0 }} formats</span>
              </div>
              <div class="ext-grid">
                <span
                  v-for="item in associations?.video ?? []"
                  :key="item.ext"
                  class="ext-chip"
                  :class="item.registered ? 'is-on' : ''"
                >.{{ item.ext }}</span>
              </div>
            </div>

            <div class="assoc-group">
              <div class="assoc-group-title">
                Your custom extensions <span>Only these can be removed</span>
              </div>
              <div class="custom-add">
                <input
                  v-model="customExt"
                  class="text-input"
                  type="text"
                  placeholder="Extension, e.g. m4b"
                  maxlength="16"
                  @keydown.enter="addCustom"
                />
                <select v-model="customKind" class="control-select">
                  <option value="audio">Audio</option>
                  <option value="video">Video</option>
                </select>
                <button
                  type="button"
                  class="btn-primary"
                  :disabled="assocBusy || !customExt.trim()"
                  @click="addCustom"
                >
                  <Plus :size="13" /> Add
                </button>
              </div>
              <div v-if="associations?.custom.length" class="custom-list">
                <div v-for="item in associations.custom" :key="item.ext" class="custom-item">
                  <span class="ext-chip" :class="item.registered ? 'is-on' : ''">.{{ item.ext }}</span>
                  <span class="custom-kind">{{ item.kind }}</span>
                  <button
                    type="button"
                    class="icon-btn"
                    :disabled="assocBusy"
                    :data-tip="`Remove .${item.ext}`"
                    @click="removeCustom(item.ext)"
                  >
                    <Trash2 :size="13" />
                  </button>
                </div>
              </div>
              <p v-else class="footnote">No custom extensions added yet.</p>
            </div>

            <p class="footnote">
              Built-in formats apply to every installation and cannot be removed individually.
              You can only remove extensions you added yourself. A leading dot is optional.
            </p>
          </div>
        </section>

        <!-- Live captions -->
        <section v-else-if="activeTab === 'captions'">
          <CaptionSettings />
        </section>

        <!-- AI upscaling -->
        <section v-else-if="activeTab === 'upscalers'">
          <UpscalerSettings />
        </section>

        <!-- Keyboard shortcuts -->
        <section v-else-if="activeTab === 'shortcuts'" class="su-panel settings-section">
          <div class="panel-body">
            <div class="shortcuts-grid">
              <div v-for="entry in shortcutLabels" :key="entry.key" class="shortcut-item">
                <span class="setting-name">{{ entry.label }}</span>
                <button
                  type="button"
                  class="key-btn"
                  :class="capturing === entry.key ? 'capturing' : ''"
                  @click="capturing = entry.key"
                  @keydown="capturing === entry.key && capture($event, entry.key)"
                >
                  {{ capturing === entry.key ? 'Press a key…' : settings.shortcuts[entry.key] }}
                </button>
              </div>
            </div>
            <p class="footnote">Click a shortcut, then press the key combination. Press Esc to cancel.</p>
          </div>
        </section>

        <!-- About -->
        <section v-else class="su-panel settings-section">
          <div class="panel-body">
            <div class="about-line"><Info :size="15" /> <span>Slide Up · built with Tauri 2, Rust and libmpv</span></div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">Version</div>
                <div class="setting-hint">Desktop media player</div>
              </div>
              <span class="version-badge">1.0.0</span>
            </div>
            <div class="setting-row">
              <div class="setting-info">
                <div class="setting-name">About & License View</div>
                <div class="setting-hint">Open the dedicated about, architecture components and open source license view.</div>
              </div>
              <div class="setting-control">
                <button type="button" class="btn-outline" @click="emit('navigate', 'about')">
                  Open About & License
                </button>
              </div>
            </div>
            <span v-if="settings.saving" class="saving">Saving…</span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* Full-width shell: fixed header on top, tab rail + content below */
.page-header { display: flex; flex-shrink: 0; align-items: center; justify-content: space-between; gap: 1rem; margin-bottom: 1.1rem; }
.page-heading { display: flex; align-items: center; gap: 0.85rem; }
.page-icon { display: flex; align-items: center; justify-content: center; width: 2.75rem; height: 2.75rem; flex-shrink: 0; border-radius: 0.85rem; background: var(--accent-gradient); color: #fff; }
.page-heading h1 { margin: 0; font-size: 1.35rem; font-weight: 700; line-height: 1.2; letter-spacing: -0.02em; color: var(--text-primary); }
.page-heading p { margin: 0.15rem 0 0; font-size: 0.75rem; color: var(--text-muted); }

.settings-layout { display: flex; min-height: 0; flex: 1; gap: 1.25rem; }

/* Left vertical tab rail */
.tabs-panel { display: flex; width: 224px; flex-shrink: 0; flex-direction: column; gap: 0.2rem; overflow-y: auto; padding: 0.6rem; align-self: flex-start; }
.tab-btn { position: relative; display: flex; width: 100%; align-items: center; gap: 0.7rem; border-radius: 0.65rem; padding: 0.6rem 0.7rem; text-align: left; font-size: 0.78rem; font-weight: 500; color: var(--text-secondary); transition: background var(--t-fast), color var(--t-fast); }
.tab-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
.tab-icon { flex-shrink: 0; opacity: 0.85; }
.tab-active { background: var(--accent-gradient-soft); color: var(--text-primary); }
.tab-active::before { content: ""; position: absolute; left: 0; top: 20%; bottom: 20%; width: 3px; border-radius: 999px; background: var(--accent-purple); }
.tab-active .tab-icon { opacity: 1; color: var(--accent-purple-soft); }

/* Active section content fills remaining width */
.content-panel { flex: 1; min-width: 0; overflow-y: auto; padding-right: 0.25rem; }
.content-head { display: flex; align-items: flex-end; justify-content: space-between; gap: 1rem; margin-bottom: 0.9rem; }
.content-head h2 { margin: 0; font-size: 1.05rem; font-weight: 700; color: var(--text-primary); }
.content-head p { margin: 0.2rem 0 0; font-size: 0.72rem; color: var(--text-muted); }
.link-btn { flex-shrink: 0; font-size: 0.72rem; font-weight: 600; color: var(--accent-purple-soft); }
.link-btn:hover { text-decoration: underline; }

.settings-section { overflow: hidden; }
.panel-body { padding: 0.2rem 1.25rem 0.5rem; }

/* Two-column setting rows: labels left, controls aligned right */
.setting-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 1.5rem; padding: 0.8rem 0; border-bottom: 1px solid var(--border-subtle); }
.setting-row:last-child { border-bottom: 0; }
.setting-info { min-width: 0; }
.setting-name { font-size: 0.82rem; font-weight: 600; color: var(--text-primary); }
.setting-hint { margin-top: 0.15rem; font-size: 0.7rem; line-height: 1.45; color: var(--text-muted); }
.setting-control { display: flex; align-items: center; justify-content: flex-end; gap: 0.5rem; justify-self: end; }
.footnote { margin: 0.2rem 0 0.5rem; font-size: 0.68rem; line-height: 1.4; color: var(--text-muted); }
.unit { font-size: 0.7rem; color: var(--text-muted); }

/* Controls */
.btn-outline { display: flex; height: 2.2rem; flex-shrink: 0; align-items: center; gap: 0.4rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.85rem; font-size: 0.72rem; color: var(--text-secondary); transition: color var(--t-fast), border-color var(--t-fast); }
.btn-outline:hover { color: var(--text-primary); border-color: var(--border-strong); }
.btn-primary { display: flex; height: 2.2rem; flex-shrink: 0; align-items: center; justify-content: center; gap: 0.4rem; border-radius: 0.6rem; background: var(--accent-gradient); padding: 0 0.9rem; font-size: 0.72rem; font-weight: 600; color: #fff; transition: filter var(--t-fast); }
.btn-primary:hover:not(:disabled) { filter: brightness(1.12); }
.btn-primary:disabled, .btn-outline:disabled, .icon-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.icon-btn { display: flex; height: 2rem; width: 2rem; flex-shrink: 0; align-items: center; justify-content: center; border-radius: 0.5rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); color: var(--text-muted); transition: color var(--t-fast), border-color var(--t-fast); }
.icon-btn:hover:not(:disabled) { color: #ff8f8f; border-color: rgba(255, 143, 143, 0.5); }
.seg-btn { height: 2rem; min-width: 3.9rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.9rem; font-size: 0.72rem; color: var(--text-secondary); transition: color var(--t-fast), border-color var(--t-fast); }
.seg-btn:hover { color: var(--text-primary); border-color: var(--border-strong); }
.seg-active { border-color: var(--accent-purple); background: rgba(135, 63, 255, 0.18); color: var(--text-primary); }
.text-input { height: 2.2rem; width: 12rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.7rem; font-size: 0.75rem; color: var(--text-primary); outline: none; }
.control-select { height: 2.2rem; min-width: 11rem; max-width: 16rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.6rem; font-size: 0.72rem; color: var(--text-secondary); outline: none; }
.color-input { width: 3.4rem; height: 2.2rem; cursor: pointer; border: 1px solid var(--border-subtle); border-radius: 0.6rem; background: var(--bg-elevated); padding: 0.2rem; }
.text-input:focus, .control-select:focus, .color-input:focus { border-color: var(--accent-purple); }

/* Toggle switch */
.toggle { appearance: none; width: 2.6rem; height: 1.45rem; border-radius: 999px; background: var(--bg-active); position: relative; cursor: pointer; transition: background var(--t-fast); }
.toggle::after { content: ""; position: absolute; top: 0.18rem; left: 0.18rem; width: 1.1rem; height: 1.1rem; border-radius: 999px; background: var(--text-secondary); transition: transform var(--t-fast), background var(--t-fast); }
.toggle:checked { background: var(--accent-purple); }
.toggle:checked::after { transform: translateX(1.15rem); background: #fff; }

/* Keyboard shortcut grid */
.shortcuts-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: 2.5rem; }
.shortcut-item { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: 0.5rem 0; border-bottom: 1px solid var(--border-subtle); }
.key-btn { min-width: 6.9rem; height: 2rem; border-radius: 0.6rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.7rem; font-size: 0.7rem; font-weight: 600; color: var(--text-secondary); transition: color var(--t-fast), border-color var(--t-fast); }
.key-btn:hover { color: var(--text-primary); border-color: var(--border-strong); }
.key-btn.capturing { border-color: var(--accent-purple); background: rgba(135, 63, 255, 0.18); color: var(--text-primary); }

/* File associations */
.assoc-body { display: flex; flex-direction: column; gap: 1.1rem; padding-top: 1rem; }
.assoc-status { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
.status-pill { display: inline-flex; height: 1.7rem; align-items: center; gap: 0.3rem; border-radius: 999px; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0 0.7rem; font-size: 0.68rem; color: var(--text-muted); }
.status-pill.is-on { border-color: rgba(40, 184, 255, 0.4); background: rgba(40, 184, 255, 0.12); color: #7dd3ff; }
.assoc-actions { display: flex; gap: 0.6rem; }
.assoc-group { display: flex; flex-direction: column; gap: 0.6rem; }
.assoc-group-title { display: flex; align-items: baseline; justify-content: space-between; gap: 1rem; font-size: 0.78rem; font-weight: 600; color: var(--text-primary); }
.assoc-group-title span { font-size: 0.67rem; font-weight: 400; color: var(--text-muted); }
.ext-grid { display: flex; flex-wrap: wrap; gap: 0.4rem; }
.ext-chip { display: inline-flex; align-items: center; border-radius: 0.4rem; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0.22rem 0.5rem; font-size: 0.68rem; color: var(--text-muted); }
.ext-chip.is-on { border-color: rgba(135, 63, 255, 0.45); background: rgba(135, 63, 255, 0.14); color: var(--text-primary); }
.custom-add { display: flex; gap: 0.5rem; }
.custom-add .text-input { flex: 1; width: auto; }
.custom-add .control-select { min-width: 7rem; }
.custom-list { display: flex; flex-direction: column; gap: 0.4rem; }
.custom-item { display: flex; align-items: center; gap: 0.7rem; padding: 0.4rem 0.5rem; border-radius: 0.5rem; background: var(--bg-base); }
.custom-item .custom-kind { flex: 1; font-size: 0.68rem; color: var(--text-muted); }

.dialog-transparent { background: rgba(16, 22, 43, 0.55) !important; backdrop-filter: blur(12px); }

/* About */
.about-line { display: flex; align-items: center; gap: 0.5rem; padding: 0.6rem 0; font-size: 0.78rem; color: var(--text-secondary); }
.version-badge { border-radius: 999px; border: 1px solid var(--border-subtle); background: var(--bg-elevated); padding: 0.2rem 0.7rem; font-size: 0.7rem; color: var(--text-secondary); }
.saving { display: inline-block; margin-top: 0.5rem; font-size: 0.7rem; color: var(--accent-purple-soft); }
</style>
