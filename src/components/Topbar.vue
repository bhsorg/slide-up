<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../services/tauri";
import AppLogo from "./AppLogo.vue";
import { Search, Sun, Moon, Minus, Square, X, X as ClearIcon, Settings, RefreshCw, Info, Mic, MicOff, DownloadCloud, Sparkles } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { useSettingsStore } from "../stores/settings";
import { useToast } from "../composables/useToast";
import type { ViewId } from "../types/settings";

const emit = defineEmits<{ navigate: [view: ViewId] }>();

const library = useLibraryStore();
const player = usePlayerStore();
const settings = useSettingsStore();
const toast = useToast();

const isLight = computed(() => settings.theme === "light");

const menuOpen = ref(false);
const menuRef = ref<HTMLElement | null>(null);

function onWindowClick(e: MouseEvent) {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    menuOpen.value = false;
  }
}

onMounted(() => {
  window.addEventListener("click", onWindowClick);
  // Silent check for update 3 seconds after boot
  setTimeout(() => {
    void checkUpdate(false);
  }, 3000);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", onWindowClick);
});

function openSettings(tab: string = "appearance") {
  menuOpen.value = false;
  settings.activeTab = tab;
  emit("navigate", "settings");
}

function openAbout() {
  menuOpen.value = false;
  emit("navigate", "about");
}

// Updater state & dialog
const updateAvailable = ref(false);
const updateModalOpen = ref(false);
const newVersion = ref("");
const updateNotes = ref("");
const isUpdating = ref(false);
const updateProgress = ref(0);

/** Dotted semver comparison; string comparison breaks on 0.10.0 vs 0.9.0. */
function isNewerVersion(candidate: string, current: string): boolean {
  const parse = (value: string) =>
    value
      .replace(/^v/i, "")
      .split(".")
      .slice(0, 3)
      .map((part) => Number.parseInt(part, 10) || 0);
  const next = parse(candidate);
  const running = parse(current);
  for (let i = 0; i < 3; i += 1) {
    if (next[i] !== running[i]) return next[i] > running[i];
  }
  return false;
}

const RELEASES_API = "https://api.github.com/repos/bhsorg/slide-up";
// Older releases shipped this text as a placeholder instead of a changelog.
const PLACEHOLDER_NOTES = "See full commit history and release notes.";

function asReleaseTag(version: string): string {
  return version.startsWith("v") ? version : `v${version}`;
}

/** Build a short change list from GitHub when a release has no useful body. */
async function fetchCommitHighlights(tag: string): Promise<string> {
  try {
    const headers = { Accept: "application/vnd.github.v3+json" };
    let commitsUrl = `${RELEASES_API}/commits?per_page=12`;

    const releasesRes = await fetch(`${RELEASES_API}/releases?per_page=10`, { headers });
    if (releasesRes.ok) {
      const releases: Array<{ tag_name?: string }> = await releasesRes.json();
      const previous = releases.find(
        (release) => release.tag_name && release.tag_name !== tag,
      )?.tag_name;
      if (previous) {
        commitsUrl = `${RELEASES_API}/compare/${encodeURIComponent(previous)}...${encodeURIComponent(tag)}`;
      }
    }

    const res = await fetch(commitsUrl, { headers });
    if (!res.ok) return "";
    const data = await res.json();
    const commits: Array<{ commit?: { message?: string } }> = Array.isArray(data?.commits)
      ? data.commits
      : Array.isArray(data)
        ? data
        : [];
    const lines = commits
      .slice(0, 12)
      .map((item) => (item.commit?.message || "").split("\n")[0].trim())
      .filter(Boolean)
      .map((message) => `• ${message}`);
    return lines.length ? `Recent changes:\n${lines.join("\n")}` : "";
  } catch {
    return "";
  }
}

async function buildUpdateNotes(tag: string, body?: string | null): Promise<string> {
  const text = (body || "").trim();
  if (text && text !== PLACEHOLDER_NOTES) return text;
  const commits = await fetchCommitHighlights(asReleaseTag(tag));
  return commits || text || "A new performance and feature update is ready for Slide Up Desktop.";
}

async function checkUpdate(manual = true) {
  if (manual) {
    menuOpen.value = false;
    toast.info("Checking for updates...", "Update");
  }

  // 1. Official Tauri updater. The endpoint 404s until a release publishes
  //    latest.json, so keep this failure isolated and still try the fallback.
  let updaterReachable = false;
  try {
    const updaterMod = await import("@tauri-apps/plugin-updater").catch(() => null);
    if (updaterMod?.check) {
      const update = await updaterMod.check();
      updaterReachable = true;
      if (update?.available) {
        updateAvailable.value = true;
        newVersion.value = update.version;
        updateNotes.value = await buildUpdateNotes(update.version, update.body);
        updateModalOpen.value = true;
        return;
      }
    }
  } catch (err) {
    console.warn("Tauri updater endpoint unavailable; trying GitHub releases.", err);
  }

  // 2. Fallback: GitHub Releases API, which does not need latest.json.
  let releasesReachable = false;
  try {
    const res = await fetch("https://api.github.com/repos/bhsorg/slide-up/releases/latest", {
      headers: { Accept: "application/vnd.github.v3+json" }
    });
    if (res.ok) {
      releasesReachable = true;
      const data = await res.json();
      const latestTag = (data.tag_name || "").replace(/^v/, "");
      const currentVer = await api.getAppVersion().catch(() => "0.1.0");

      if (latestTag && isNewerVersion(latestTag, currentVer)) {
        updateAvailable.value = true;
        newVersion.value = latestTag;
        updateNotes.value = await buildUpdateNotes(latestTag, data.body);
        if (manual) {
          updateModalOpen.value = true;
        }
        return;
      }
    }
  } catch (err) {
    console.warn("GitHub releases check failed.", err);
  }

  if (!manual) return;

  if (updaterReachable || releasesReachable) {
    const ver = await api.getAppVersion().catch(() => "0.1.0");
    toast.success(`You are running the latest version of Slide Up (v${ver}).`, "Up to date");
  } else {
    toast.error("Could not reach update server. Check your internet connection.", "Update");
  }
}

async function startInstallUpdate() {
  isUpdating.value = true;
  toast.info("Downloading update in background...", "Update");

  try {
    const updaterMod = await import("@tauri-apps/plugin-updater").catch(() => null);
    if (updaterMod?.check) {
      const update = await updaterMod.check();
      if (update?.available) {
        let downloaded = 0;
        let contentLength = 0;
        await update.downloadAndInstall((event) => {
          if (event.event === "Started") {
            contentLength = event.data.contentLength || 0;
          } else if (event.event === "Progress") {
            downloaded += event.data.chunkLength;
            if (contentLength > 0) {
              updateProgress.value = Math.min(100, Math.round((downloaded / contentLength) * 100));
            }
          } else if (event.event === "Finished") {
            updateProgress.value = 100;
          }
        });
        toast.success("Update downloaded! Restarting to apply...", "Updated");
        const { relaunch } = await import("@tauri-apps/plugin-process");
        await relaunch();
        return;
      }
    }

    // Fallback: open GitHub release page if direct plugin binary install not configured
    await api.openExternal("https://github.com/bhsorg/slide-up/releases/latest");
    updateModalOpen.value = false;
  } catch (err) {
    toast.error(String(err), "Update installation failed");
    await api.openExternal("https://github.com/bhsorg/slide-up/releases/latest");
  } finally {
    isUpdating.value = false;
  }
}

// Voice / Microphone search using Web Speech Recognition API
const isListening = ref(false);
let recognition: any = null;

function toggleVoiceSearch() {
  const SpeechRecognition = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
  if (!SpeechRecognition) {
    toast.error("Speech recognition is not supported in this environment.", "Microphone");
    return;
  }

  if (isListening.value) {
    if (recognition) {
      recognition.stop();
    }
    isListening.value = false;
    return;
  }

  try {
    recognition = new SpeechRecognition();
    recognition.continuous = false;
    recognition.interimResults = false;
    recognition.lang = "hi-IN,en-US"; // supports Hindi and English lyrics / track search

    recognition.onstart = () => {
      isListening.value = true;
      toast.info("Listening... Speak a song title or lyrics line", "Voice Search");
    };

    recognition.onresult = (event: any) => {
      const transcript = event.results?.[0]?.[0]?.transcript;
      if (transcript) {
        library.search = transcript.trim();
        toast.success(`Searching: "${transcript.trim()}"`, "Voice Search");
      }
      isListening.value = false;
    };

    recognition.onerror = (event: any) => {
      console.warn("SpeechRecognition error:", event.error);
      if (event.error !== "no-speech") {
        toast.error(`Microphone error: ${event.error}`, "Voice Search");
      }
      isListening.value = false;
    };

    recognition.onend = () => {
      isListening.value = false;
    };

    recognition.start();
  } catch (err) {
    console.error("Failed to start voice recognition:", err);
    toast.error("Could not access microphone.", "Voice Search");
    isListening.value = false;
  }
}

async function windowAction(action: "minimize" | "maximize" | "close") {
  try {
    const win = getCurrentWindow();
    if (action === "minimize") {
      await win.minimize();
      await api.showWidgetWhenMainHidden();
    } else if (action === "maximize") {
      await win.toggleMaximize();
    } else if (settings.settings.keepWidgetOnClose && settings.settings.playerWidget) {
      await win.hide();
      toast.info("Player widget stays on your desktop. Use it or the tray icon to restore.", "Slide Up");
    } else if (settings.settings.taskbarControls && player.isPlaying && !player.isVideo) {
      await win.minimize();
      toast.info("Music is playing in background. Use taskbar controls.", "Slide Up");
    } else if (settings.settings.minimizeToTray) {
      await win.hide();
    } else {
      await win.close();
    }
  } catch (error) {
    // Keep the user-facing message simple, but log the real IPC error so a
    // missing capability or failed window command is diagnosable.
    console.error("[Topbar] window action failed:", error);
    toast.info("Window controls are available in the desktop app.", "Slide Up");
  }
}
</script>

<template>
  <header
    class="relative flex h-[var(--topbar-height)] shrink-0 items-center border-b border-line bg-secondary px-3 select-none">
    <!-- Left: Brand & Logo aligned with Sidebar width -->
    <div class="flex w-[var(--sidebar-width)] shrink-0 items-center gap-2.5 pr-2 z-10">
      <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg overflow-hidden">
        <AppLogo class="h-7 w-7 object-contain" />
      </div>
      <div class="min-w-0">
        <div class="truncate text-[14px] font-bold tracking-tight text-ink">Slide Up</div>
        <div class="truncate text-[10px] text-faint">Play · Enjoy · Repeat</div>
      </div>
    </div>

    <!-- Center: Search input absolutely centered relative to the whole window -->
    <div class="pointer-events-none absolute inset-x-0 top-0 flex h-full items-center justify-center z-20">
      <div class="pointer-events-auto relative w-full max-w-[460px] px-4">
        <Search :size="15" class="pointer-events-none absolute left-7.5 top-1/2 -translate-y-1/2 text-faint" />
        <input
          v-model="library.search"
          type="search"
placeholder="Search media, artists, albums, or lyrics..."
          class="h-8.5 w-full rounded-lg border border-line bg-panel pl-9.5 pr-16 text-[12px] text-ink outline-none transition-colors placeholder:text-faint focus:border-purple/60 focus:bg-elevated"
        />
        <div class="absolute right-6 top-1/2 -translate-y-1/2 flex items-center gap-1">
          <button v-if="library.search" type="button" class="rounded-md p-0.5 text-faint hover:bg-hover hover:text-ink"
            title="Clear search" @click="library.search = ''">
            <ClearIcon :size="13" />
          </button>
          <button type="button" class="rounded-md p-1 transition-colors"
            :class="isListening ? 'bg-red-500/20 text-red-400 animate-pulse' : 'text-faint hover:bg-hover hover:text-ink'"
            :title="isListening ? 'Listening... click to stop' : 'Search by voice / microphone (Song or lyrics)'"
            @click="toggleVoiceSearch">
            <Mic v-if="!isListening" :size="14" />
            <MicOff v-else :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- Frameless window: drag handle filling remaining space -->
    <div data-tauri-drag-region class="h-full flex-1 self-stretch"></div>

    <!-- Right: Theme switcher & Window Controls -->
    <div class="ml-auto flex shrink-0 items-center gap-2 z-10">
      <!-- Theme switcher -->
      <div class="flex items-center rounded-full border border-line bg-panel p-0.5">
        <button
          type="button"
          class="flex h-6.5 w-6.5 items-center justify-center rounded-full transition-colors"
          :class="isLight ? 'bg-amber-400 text-black' : 'text-amber-400/70 hover:text-amber-400'"
          title="Light theme"
          @click="settings.setTheme('light')"
        >
          <Sun :size="13" />
        </button>
        <button
          type="button"
          class="flex h-6.5 w-6.5 items-center justify-center rounded-full transition-colors"
          :class="!isLight ? 'bg-purple text-white' : 'text-faint hover:text-ink'"
          title="Dark theme"
          @click="settings.setTheme('dark')"
        >
          <Moon :size="13" />
        </button>
      </div>

      <!-- Settings Dropdown Menu (Right of Theme Switcher) -->
      <div ref="menuRef" class="relative">
        <button
          type="button"
          class="relative flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted transition-colors hover:bg-hover hover:text-ink cursor-pointer"
          :class="{ 'bg-hover text-ink': menuOpen }"
          title="Settings & More"
          @click.stop="menuOpen = !menuOpen"
        >
          <Settings :size="15" />
          <!-- Update available notification badge -->
          <span
            v-if="updateAvailable"
            class="absolute top-1 right-1 h-2 w-2 rounded-full bg-emerald-400 ring-2 ring-secondary animate-pulse"
            title="Update available"
          />
        </button>

        <!-- Dropdown Menu -->
        <Transition name="fade-scale">
          <div
            v-if="menuOpen"
            class="absolute right-0 top-full mt-1.5 w-48 z-50 rounded-xl border border-line-strong bg-elevated/95 p-1.5 shadow-xl backdrop-blur-md"
            @click.stop
          >
            <button
              type="button"
              class="topbar-menu-item"
              @click="openSettings('appearance')"
            >
              <Settings :size="14" class="text-purple-400" />
              <span>Setting</span>
            </button>

            <button
              type="button"
              class="topbar-menu-item relative flex items-center justify-between"
              @click="checkUpdate(true)"
            >
              <div class="flex items-center gap-2">
                <RefreshCw :size="14" class="text-blue-400" :class="{ 'animate-spin': isUpdating }" />
                <span>Check Update</span>
              </div>
              <span v-if="updateAvailable" class="rounded bg-emerald-500/20 px-1 py-0.2 text-[9px] font-bold text-emerald-400">
                NEW
              </span>
            </button>

            <div class="my-1 h-px bg-line" />

            <button
              type="button"
              class="topbar-menu-item"
              @click="openAbout"
            >
              <Info :size="14" class="text-emerald-400" />
              <span>About / License</span>
            </button>
          </div>
        </Transition>
      </div>

      <div class="flex items-center gap-0.5 pl-1">
        <button
          type="button"
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted hover:bg-hover hover:text-ink"
          title="Minimize"
          @click="windowAction('minimize')"
        >
          <Minus :size="14" />
        </button>
        <button
          type="button"
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted hover:bg-hover hover:text-ink"
          title="Maximize"
          @click="windowAction('maximize')"
        >
          <Square :size="12" />
        </button>
        <button
          type="button"
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted hover:bg-[#e0446a] hover:text-white"
          title="Close"
          @click="windowAction('close')"
        >
          <X :size="14" />
        </button>
      </div>
    </div>
  </header>

  <!-- Modern In-App Update Modal Dialog -->
  <Teleport to="body">
    <Transition name="fade-scale">
      <div
        v-if="updateModalOpen"
        class="fixed inset-0 z-[99999] flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
        @click.self="!isUpdating ? (updateModalOpen = false) : null"
      >
        <div class="relative w-full max-w-md rounded-2xl border border-line-strong bg-elevated p-6 shadow-2xl space-y-4">
          <!-- Modal Header -->
          <div class="flex items-start justify-between gap-3">
            <div class="flex items-center gap-3">
              <div class="flex h-11 w-11 items-center justify-center rounded-xl bg-purple/20 text-purple-400">
                <Sparkles :size="22" />
              </div>
              <div>
                <h3 class="text-base font-bold text-ink">Update Available!</h3>
                <p class="text-xs text-muted">A new version of Slide Up is ready</p>
              </div>
            </div>
            <button
              v-if="!isUpdating"
              type="button"
              class="rounded-lg p-1 text-faint hover:bg-hover hover:text-ink transition-colors"
              title="Close"
              @click="updateModalOpen = false"
            >
              <ClearIcon :size="16" />
            </button>
          </div>

          <!-- Version info pill -->
          <div class="flex items-center justify-between rounded-xl border border-line bg-panel p-3">
            <span class="text-xs text-muted">Target Release:</span>
            <span class="rounded-md bg-emerald-500/20 px-2 py-0.5 text-xs font-bold text-emerald-400">
              v{{ newVersion }}
            </span>
          </div>

          <!-- Release Notes -->
          <div class="rounded-xl border border-line/60 bg-base/50 p-3 max-h-36 overflow-y-auto custom-scrollbar">
            <p class="text-[11px] font-semibold text-faint uppercase mb-1">Release Highlights</p>
            <p class="text-xs text-ink/90 whitespace-pre-wrap leading-relaxed">{{ updateNotes }}</p>
          </div>

          <!-- Progress Bar if updating -->
          <div v-if="isUpdating" class="space-y-1.5 pt-1">
            <div class="flex justify-between text-[11px] text-muted">
              <span>Downloading update…</span>
              <span>{{ updateProgress }}%</span>
            </div>
            <div class="h-2 w-full overflow-hidden rounded-full bg-panel">
              <div
                class="h-full bg-linear-to-r from-purple to-blue transition-all duration-200"
                :style="{ width: `${updateProgress}%` }"
              />
            </div>
          </div>

          <!-- Actions -->
          <div class="flex items-center justify-end gap-2.5 pt-2">
            <button
              v-if="!isUpdating"
              type="button"
              class="px-4 py-2 rounded-xl border border-line text-xs font-medium text-muted hover:bg-hover hover:text-ink transition-colors"
              @click="updateModalOpen = false"
            >
              Later
            </button>
            <button
              type="button"
              class="inline-flex items-center gap-2 px-4 py-2 rounded-xl bg-purple text-xs font-semibold text-white shadow-lg shadow-purple/30 hover:bg-purple-600 transition-colors cursor-pointer disabled:opacity-60"
              :disabled="isUpdating"
              @click="startInstallUpdate"
            >
              <DownloadCloud :size="15" :class="{ 'animate-bounce': !isUpdating, 'animate-pulse': isUpdating }" />
              <span>{{ isUpdating ? 'Installing…' : 'Update & Restart' }}</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
input[type="search"]::-webkit-search-cancel-button,
input[type="search"]::-webkit-search-decoration,
input[type="search"]::-webkit-search-results-button,
input[type="search"]::-webkit-search-results-decoration {
  -webkit-appearance: none;
  appearance: none;
  display: none;
}

.topbar-menu-item {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 0.65rem;
  border-radius: 0.5rem;
  padding: 0.45rem 0.65rem;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  transition: background var(--t-fast);
}

.topbar-menu-item:hover {
  background: var(--bg-hover);
  color: var(--text-ink);
}

.fade-scale-enter-active,
.fade-scale-leave-active {
  transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-scale-enter-from,
.fade-scale-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.96);
}
</style>
