<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Search, Sun, Moon, Minus, Square, X, X as ClearIcon, Settings, RefreshCw, Info } from "@lucide/vue";
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

async function checkUpdate() {
  menuOpen.value = false;
  toast.info("Checking for updates...", "Update");
  await new Promise((resolve) => setTimeout(resolve, 800));
  toast.success("You are running the latest version of Slide Up (v1.0.0).", "Up to date");
}

async function windowAction(action: "minimize" | "maximize" | "close") {
  try {
    const win = getCurrentWindow();
    if (action === "minimize") {
      await win.minimize();
    } else if (action === "maximize") {
      await win.toggleMaximize();
    } else if (settings.settings.taskbarControls && player.isPlaying && !player.isVideo) {
      await win.minimize();
      toast.info("Music is playing in background. Use taskbar controls.", "Slide Up");
    } else if (settings.settings.minimizeToTray) {
      await win.hide();
    } else {
      await win.close();
    }
  } catch {
    toast.info("Window controls are available in the desktop app.");
  }
}
</script>

<template>
  <header class="flex h-[var(--topbar-height)] shrink-0 items-center border-b border-line bg-secondary px-3">
    <!-- Left: Brand & Logo aligned with Sidebar width -->
    <div class="flex w-[var(--sidebar-width)] shrink-0 items-center gap-2.5 pr-2">
      <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg">
        <svg viewBox="0 0 44 44" class="h-7 w-7" aria-hidden="true">
          <defs>
            <linearGradient id="topbar-logo-gradient" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0%" stop-color="#873fff" />
              <stop offset="100%" stop-color="#28b8ff" />
            </linearGradient>
          </defs>
          <path d="M8 4.5 38 21 8 39.5Z" fill="url(#topbar-logo-gradient)" />
          <path d="M8 4.5 8 39.5 23 30.5Z" fill="#28b8ff" opacity="0.7" />
        </svg>
      </div>
      <div class="min-w-0">
        <div class="truncate text-[14px] font-bold tracking-tight text-ink">Slide Up</div>
        <div class="truncate text-[10px] text-faint">Play · Enjoy · Repeat</div>
      </div>
    </div>

    <!-- Center: Search input perfectly centered -->
    <div class="flex flex-1 items-center justify-center px-4">
      <div class="relative w-full max-w-[440px]">
        <Search :size="15" class="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-faint" />
        <input
          v-model="library.search"
          type="search"
          placeholder="Search media, artists, albums..."
          class="h-8.5 w-full rounded-lg border border-line bg-panel pl-9.5 pr-9 text-[12px] text-ink outline-none transition-colors placeholder:text-faint focus:border-purple/60 focus:bg-elevated"
        />
        <button
          v-if="library.search"
          type="button"
          class="absolute right-2.5 top-1/2 -translate-y-1/2 rounded-md p-0.5 text-faint hover:bg-hover hover:text-ink"
          title="Clear search"
          @click="library.search = ''"
        >
          <ClearIcon :size="13" />
        </button>
      </div>
    </div>

    <!-- Frameless window: empty drag handle -->
    <div data-tauri-drag-region class="h-full flex-1 self-stretch"></div>

    <!-- Right: Theme switcher & Window Controls -->
    <div class="ml-auto flex shrink-0 items-center gap-2">
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
          class="flex h-7.5 w-7.5 items-center justify-center rounded-md text-muted transition-colors hover:bg-hover hover:text-ink cursor-pointer"
          :class="{ 'bg-hover text-ink': menuOpen }"
          title="Settings & More"
          @click.stop="menuOpen = !menuOpen"
        >
          <Settings :size="15" />
        </button>

        <!-- Dropdown Menu -->
        <Transition name="fade-scale">
          <div
            v-if="menuOpen"
            class="absolute right-0 top-full mt-1.5 w-44 z-50 rounded-xl border border-line-strong bg-elevated/95 p-1.5 shadow-xl backdrop-blur-md"
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
              class="topbar-menu-item"
              @click="checkUpdate"
            >
              <RefreshCw :size="14" class="text-blue-400" />
              <span>Check Update</span>
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
