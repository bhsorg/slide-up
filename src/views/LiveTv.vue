<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  Tv, Play, Search, Grid, List, Plus, Trash2, RefreshCw,
  Globe, Check, ChevronDown, AlertCircle
} from "@lucide/vue";
import { usePlayerStore } from "../stores/player";
import { useToast } from "../composables/useToast";
import type { MediaItem } from "../types/media";

const emit = defineEmits<{ (e: "open-player"): void }>();

const player = usePlayerStore();
const toast = useToast();

export interface IptvChannel {
  id: string;
  name: string;
  logo: string;
  url: string;
  category: string;
  language: string;
  country: string;
}

export interface IptvPlaylist {
  id: string;
  name: string;
  url: string;
  channelCount?: number;
  addedAt: number;
}

// Persistent state
const STORAGE_PLAYLISTS = "slideup_iptv_saved_playlists";
const STORAGE_ACTIVE_URL = "slideup_iptv_active_url";
const STORAGE_CACHED_CHANNELS = "slideup_iptv_cached_channels";
const STORAGE_VIEW_MODE = "slideup_iptv_view_mode";
const STORAGE_LEGACY_PURGE_FLAG = "slideup_iptv_legacy_purged_v1";

const currentUrl = ref<string>("");
const savedPlaylists = ref<IptvPlaylist[]>([]);
const channels = ref<IptvChannel[]>([]);
const loading = ref<boolean>(false);
const errorMsg = ref<string | null>(null);

// UI controls
const viewMode = ref<"grid" | "list">("grid");
const showUrlModal = ref<boolean>(false);
const newPlaylistName = ref<string>("");
const newPlaylistUrl = ref<string>("");

// Filters
const searchQuery = ref<string>("");
const selectedLanguage = ref<string>("all");
const selectedCategory = ref<string>("all");

// Parsing M3U content
function parseM3u(content: string): IptvChannel[] {
  const lines = content.split(/\r?\n/);
  const result: IptvChannel[] = [];
  let currentMeta: Partial<IptvChannel> | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) continue;

    if (line.startsWith("#EXTINF:")) {
      const infoStr = line.substring(8);
      const commaIdx = infoStr.lastIndexOf(",");
      const title = commaIdx !== -1 ? infoStr.substring(commaIdx + 1).trim() : "Channel";
      const meta = commaIdx !== -1 ? infoStr.substring(0, commaIdx) : infoStr;

      const getAttr = (attr: string): string => {
        const match = meta.match(new RegExp(`${attr}="([^"]*)"`, "i"));
        return match ? match[1].trim() : "";
      };

      const name = getAttr("tvg-name") || title;
      const logo = getAttr("tvg-logo");
      const category = getAttr("group-title") || "General";
      const language = getAttr("tvg-language") || getAttr("language") || "All";
      const country = getAttr("tvg-country") || "";

      currentMeta = {
        name: name || title,
        logo,
        category: category || "General",
        language: language || "All",
        country,
      };
    } else if (!line.startsWith("#") && currentMeta) {
      if (
        line.startsWith("http://") ||
        line.startsWith("https://") ||
        line.startsWith("rtmp://") ||
        line.startsWith("rtsp://")
      ) {
        result.push({
          id: `ch-${result.length + 1}-${currentMeta.name?.slice(0, 15).replace(/\s+/g, "_")}`,
          name: currentMeta.name || `Channel ${result.length + 1}`,
          logo: currentMeta.logo || "",
          url: line,
          category: currentMeta.category || "General",
          language: currentMeta.language || "All",
          country: currentMeta.country || "",
        });
        currentMeta = null;
      }
    }
  }
  return result;
}

// Fetch & load playlist
async function loadPlaylist(url: string, force = false) {
  if (!url) return;
  currentUrl.value = url;
  localStorage.setItem(STORAGE_ACTIVE_URL, url);

  // Use cache if same URL and not forced
  if (!force && channels.value.length > 0) {
    const cachedUrl = localStorage.getItem(STORAGE_ACTIVE_URL);
    if (cachedUrl === url) return;
  }

  loading.value = true;
  errorMsg.value = null;

  try {
    const response = await fetch(url, { headers: { Accept: "*/*" } });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }
    const text = await response.text();
    const parsed = parseM3u(text);

    if (parsed.length === 0) {
      throw new Error("No channels found in this M3U playlist.");
    }

    channels.value = parsed;
    // Cache up to first 800 channels for instant next startup
    try {
      localStorage.setItem(STORAGE_CACHED_CHANNELS, JSON.stringify(parsed.slice(0, 600)));
    } catch {
      // localStorage quota exceeded fallback
    }

    // Update channel count in saved playlist list
    const found = savedPlaylists.value.find((p) => p.url === url);
    if (found) {
      found.channelCount = parsed.length;
      savePlaylistsToStorage();
    }

    toast.success(`Loaded ${parsed.length} live channels`, "IPTV");
  } catch (err) {
    const msg = String(err);
    errorMsg.value = msg;
    toast.error(msg, "IPTV Load Failed");
  } finally {
    loading.value = false;
  }
}

// Save playlist URL to user presets
function saveCustomPlaylist() {
  const url = newPlaylistUrl.value.trim();
  const name = newPlaylistName.value.trim() || `Playlist ${savedPlaylists.value.length + 1}`;
  if (!url) {
    toast.error("Please enter a valid M3U playlist URL");
    return;
  }

  const existing = savedPlaylists.value.find((p) => p.url === url);
  if (existing) {
    existing.name = name;
  } else {
    savedPlaylists.value.unshift({
      id: `custom-${Date.now()}`,
      name,
      url,
      channelCount: 0,
      addedAt: Date.now(),
    });
  }

  savePlaylistsToStorage();
  newPlaylistName.value = "";
  newPlaylistUrl.value = "";
  showUrlModal.value = false;
  void loadPlaylist(url, true);
}

function removePlaylist(id: string) {
  const pl = savedPlaylists.value.find((p) => p.id === id);
  savedPlaylists.value = savedPlaylists.value.filter((p) => p.id !== id);
  savePlaylistsToStorage();
  if (pl && pl.url === currentUrl.value) {
    currentUrl.value = "";
    channels.value = [];
    localStorage.removeItem(STORAGE_ACTIVE_URL);
    localStorage.removeItem(STORAGE_CACHED_CHANNELS);
  }
  toast.info("Playlist removed from saved list");
}

function savePlaylistsToStorage() {
  localStorage.setItem(STORAGE_PLAYLISTS, JSON.stringify(savedPlaylists.value));
}

function channelToMediaItem(ch: IptvChannel, idx: number): MediaItem {
  return {
    id: -10000 - idx,
    path: ch.url,
    kind: "video",
    title: ch.name,
    artist: ch.category || "Live TV",
    album: ch.language && ch.language !== "All" ? ch.language : "Live TV",
    durationMs: 0,
    codec: "Live HLS",
    width: null,
    height: null,
    sizeBytes: 0,
    thumbnailPath: ch.logo || null,
    favorite: false,
    playCount: 0,
    lastPlayedAt: Date.now(),
    addedAt: Date.now(),
  };
}

// Play channel stream and populate the queue so next/previous buttons navigate channels
async function playChannel(channel: IptvChannel) {
  const list = filteredChannels.value.length > 0 ? filteredChannels.value : channels.value;
  const targetIndex = list.findIndex((c) => c.url === channel.url);
  const playIdx = targetIndex >= 0 ? targetIndex : 0;
  const queueItems: MediaItem[] = list.map((ch, idx) => channelToMediaItem(ch, idx));
  const playlistName = currentPlaylistTitle.value || "Live TV";

  try {
    toast.info(`Connecting to ${channel.name}…`, "Live TV");
    await player.playList(queueItems, playIdx, `${playlistName} (${queueItems.length})`);
    emit("open-player");
  } catch (err) {
    toast.error(String(err), "Playback Error");
  }
}

// Extracted unique filter lists
const availableLanguages = computed(() => {
  const set = new Set<string>();
  channels.value.forEach((ch) => {
    if (ch.language && ch.language !== "All" && ch.language !== "General") {
      ch.language.split(";").forEach((l) => set.add(l.trim()));
    }
  });
  return Array.from(set).sort();
});

const availableCategories = computed(() => {
  const set = new Set<string>();
  channels.value.forEach((ch) => {
    if (ch.category && ch.category !== "General") {
      set.add(ch.category);
    }
  });
  return Array.from(set).sort();
});

// Filtered channels
const filteredChannels = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  const lan = selectedLanguage.value.toLowerCase();
  const cat = selectedCategory.value.toLowerCase();

  return channels.value.filter((ch) => {
    if (q && !ch.name.toLowerCase().includes(q)) return false;
    if (lan !== "all" && !ch.language.toLowerCase().includes(lan)) return false;
    if (cat !== "all" && !ch.category.toLowerCase().includes(cat)) return false;
    return true;
  });
});

const currentPlaylistTitle = computed(() => {
  if (!currentUrl.value) return "Live TV Player";
  const found = savedPlaylists.value.find((p) => p.url === currentUrl.value);
  return found?.name || "Live TV Stream";
});

// Watch view mode changes
watch(viewMode, (mode) => {
  localStorage.setItem(STORAGE_VIEW_MODE, mode);
});

onMounted(() => {
  // One-time cleanup for old dev cache so existing test data doesn't persist
  if (!localStorage.getItem(STORAGE_LEGACY_PURGE_FLAG)) {
    localStorage.removeItem(STORAGE_ACTIVE_URL);
    localStorage.removeItem(STORAGE_CACHED_CHANNELS);
    const oldSaved = localStorage.getItem(STORAGE_PLAYLISTS);
    if (oldSaved) {
      try {
        const parsed = JSON.parse(oldSaved);
        const userOnly = Array.isArray(parsed)
          ? parsed.filter((p: IptvPlaylist) => !p.id.startsWith("preset-"))
          : [];
        localStorage.setItem(STORAGE_PLAYLISTS, JSON.stringify(userOnly));
      } catch {
        localStorage.removeItem(STORAGE_PLAYLISTS);
      }
    }
    localStorage.setItem(STORAGE_LEGACY_PURGE_FLAG, "true");
  }

  // Load saved playlists (user's custom saved playlists only)
  const saved = localStorage.getItem(STORAGE_PLAYLISTS);
  if (saved) {
    try {
      savedPlaylists.value = JSON.parse(saved);
    } catch {
      savedPlaylists.value = [];
    }
  }

  // Restore view mode
  const mode = localStorage.getItem(STORAGE_VIEW_MODE);
  if (mode === "grid" || mode === "list") {
    viewMode.value = mode;
  }

  // Restore active URL if user has chosen or saved one
  const activeUrl = localStorage.getItem(STORAGE_ACTIVE_URL);
  if (activeUrl) {
    currentUrl.value = activeUrl;
    const cached = localStorage.getItem(STORAGE_CACHED_CHANNELS);
    if (cached) {
      try {
        channels.value = JSON.parse(cached);
      } catch {
        // ignore
      }
    }

    // Only load if cache was empty
    if (!channels.value.length) {
      void loadPlaylist(activeUrl);
    }
  }
});
</script>

<template>
  <div class="iptv-container">
    <!-- Top Header -->
    <div class="iptv-header">
      <div class="iptv-brand">
        <div class="tv-icon-badge">
          <Tv :size="20" />
        </div>
        <div>
          <div class="flex items-center gap-2">
            <h1 class="iptv-title">{{ currentPlaylistTitle }}</h1>
            <span v-if="channels.length" class="live-pill">
              <span class="live-pulse" /> LIVE
            </span>
          </div>
          <p class="iptv-subtitle">
            {{ channels.length ? `${channels.length} channels loaded · M3U & HLS streaming` : 'Select or add an M3U playlist URL to stream live channels' }}
          </p>
        </div>
      </div>

      <!-- Header Action Controls -->
      <div class="header-actions">
        <!-- Switch / Add Playlist Button -->
        <button type="button" class="btn-iptv-action" @click="showUrlModal = !showUrlModal">
          <Globe :size="14" />
          <span>{{ currentUrl ? 'Switch Playlist' : 'Add Playlist' }}</span>
          <ChevronDown :size="13" class="opacity-70" />
        </button>

        <!-- Reload Playlist -->
        <button v-if="currentUrl" type="button" class="btn-icon" :class="{ 'animate-spin': loading }" title="Reload Current Playlist"
          :disabled="loading" @click="loadPlaylist(currentUrl, true)">
          <RefreshCw :size="15" />
        </button>

        <!-- View Mode (Grid vs List) -->
        <div class="view-toggle">
          <button type="button" class="view-toggle-btn" :class="{ active: viewMode === 'grid' }" title="Grid View"
            @click="viewMode = 'grid'">
            <Grid :size="15" />
          </button>
          <button type="button" class="view-toggle-btn" :class="{ active: viewMode === 'list' }" title="List View"
            @click="viewMode = 'list'">
            <List :size="15" />
          </button>
        </div>
      </div>
    </div>

    <!-- Filter & Search Toolbar (Only when channels are loaded) -->
    <div v-if="channels.length" class="iptv-toolbar">
      <!-- Search Input -->
      <div class="search-box">
        <Search :size="14" class="search-icon" />
        <input v-model="searchQuery" type="text" placeholder="Search channels..." class="search-input" />
        <button v-if="searchQuery" type="button" class="clear-search-btn" @click="searchQuery = ''">
          &times;
        </button>
      </div>

      <!-- Language Filter -->
      <div class="filter-select-wrapper">
        <label class="filter-label">Language:</label>
        <select v-model="selectedLanguage" class="filter-select">
          <option value="all">All Languages ({{ availableLanguages.length + 1 }})</option>
          <option v-for="lan in availableLanguages" :key="lan" :value="lan">
            {{ lan }}
          </option>
        </select>
      </div>

      <!-- Category Filter -->
      <div class="filter-select-wrapper">
        <label class="filter-label">Category:</label>
        <select v-model="selectedCategory" class="filter-select">
          <option value="all">All Categories ({{ availableCategories.length + 1 }})</option>
          <option v-for="cat in availableCategories" :key="cat" :value="cat">
            {{ cat }}
          </option>
        </select>
      </div>

      <div class="channel-count-badge">
        {{ filteredChannels.length }} / {{ channels.length }}
      </div>
    </div>

    <!-- Main Content Area: Channels -->
    <div class="iptv-content">
      <!-- Loading State -->
      <div v-if="loading" class="empty-state">
        <RefreshCw :size="32" class="animate-spin text-purple-400 mb-2" />
        <h3 class="text-ink text-base font-semibold">Loading Live Playlist...</h3>
        <p class="text-muted text-xs mt-1 max-w-sm text-center">
          Parsing channel streams and logos from {{ currentUrl }}
        </p>
      </div>

      <!-- Error State -->
      <div v-else-if="errorMsg" class="empty-state">
        <AlertCircle :size="32" class="text-rose-400 mb-2" />
        <h3 class="text-ink text-base font-semibold">Failed to Load Channels</h3>
        <p class="text-rose-400 text-xs mt-1 max-w-md text-center">{{ errorMsg }}</p>
        <button type="button"
          class="mt-3 px-3 py-1.5 rounded-lg bg-purple-600 hover:bg-purple-700 text-white text-xs font-semibold"
          @click="loadPlaylist(currentUrl, true)">
          Retry Connection
        </button>
      </div>

      <!-- Clean Initial State: No Playlist Selected -->
      <div v-else-if="!currentUrl || channels.length === 0" class="empty-state">
        <div class="p-4 rounded-2xl bg-purple-500/10 border border-purple-500/20 text-purple-400 mb-3">
          <Tv :size="40" />
        </div>
        <h3 class="text-ink text-base font-semibold">No Playlist Loaded</h3>
        <p class="text-muted text-xs mt-1 max-w-sm text-center">
          Add an M3U or HLS playlist URL to stream live channels. Slide Up does not bundle any preloaded channels.
        </p>
        <button
          type="button"
          class="mt-4 px-4 py-2 rounded-lg bg-[#8b5cf6] hover:bg-[#7c3aed] text-white text-xs font-semibold flex items-center gap-1.5 cursor-pointer shadow-md transition-colors"
          @click="showUrlModal = true"
        >
          <Plus :size="14" />
          <span>Add M3U Playlist</span>
        </button>
      </div>

      <!-- Empty Filter State (When search/filter has no match) -->
      <div v-else-if="filteredChannels.length === 0" class="empty-state">
        <Tv :size="36" class="text-muted/40 mb-2" />
        <h3 class="text-ink text-base font-semibold">No Channels Found</h3>
        <p class="text-muted text-xs mt-1">Try resetting the language, category, or search query.</p>
        <button type="button" class="mt-3 px-3 py-1 rounded border border-line bg-panel hover:bg-hover text-ink text-xs transition-colors"
          @click="searchQuery = ''; selectedLanguage = 'all'; selectedCategory = 'all'">
          Clear Filters
        </button>
      </div>

      <!-- Grid View -->
      <div v-else-if="viewMode === 'grid'" class="channels-grid">
        <div
          v-for="channel in filteredChannels"
          :key="channel.id"
          class="channel-card group"
          :class="{ 'ring-2 ring-purple-500 bg-purple-500/10': player.state.current?.path === channel.url }"
          @click="playChannel(channel)"
        >
          <div class="card-logo-box relative">
            <img v-if="channel.logo" :src="channel.logo" :alt="channel.name" loading="lazy" class="channel-logo"
              @error="($event.target as HTMLImageElement).style.display = 'none'" />
            <div class="logo-fallback">
              <Tv :size="32" class="opacity-40 text-purple-300" />
            </div>

            <!-- Active Live Badge on Playing Channel -->
            <div v-if="player.state.current?.path === channel.url" class="absolute top-2 left-2 z-10 pointer-events-none">
              <span class="live-dot-badge">
                <span class="live-dot"></span>
                LIVE
              </span>
            </div>

            <!-- Hover Play Overlay -->
            <div class="card-play-overlay">
              <div class="play-hero-btn">
                <Play :size="16" fill="white" class="ml-0.5 text-white" />
              </div>
            </div>
          </div>

          <div class="card-meta">
            <h4 class="channel-name" :class="{ 'text-purple font-bold': player.state.current?.path === channel.url }" :title="channel.name">{{ channel.name }}</h4>
            <div class="channel-tags">
              <span class="tag-category">{{ channel.category }}</span>
              <span v-if="channel.language && channel.language !== 'All'" class="tag-lan">
                {{ channel.language }}
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- List View -->
      <div v-else class="channels-list">
        <div
          v-for="(channel, idx) in filteredChannels"
          :key="channel.id"
          class="list-row group"
          :class="{ 'bg-purple-500/10 border-purple-500/30': player.state.current?.path === channel.url }"
          @click="playChannel(channel)"
        >
          <span class="row-num">{{ idx + 1 }}</span>

          <div class="row-logo-box">
            <img v-if="channel.logo" :src="channel.logo" :alt="channel.name" loading="lazy" class="row-logo"
              @error="($event.target as HTMLImageElement).style.display = 'none'" />
            <Tv :size="14" class="opacity-40" />
          </div>

          <div class="row-info flex items-center gap-2">
            <span class="row-title" :class="{ 'text-purple font-bold': player.state.current?.path === channel.url }">{{ channel.name }}</span>
            <span v-if="player.state.current?.path === channel.url" class="live-dot-badge scale-90">
              <span class="live-dot"></span>
              LIVE
            </span>
          </div>

          <span class="row-category">{{ channel.category }}</span>
          <span class="row-lan">{{ channel.language !== 'All' ? channel.language : 'General' }}</span>

          <button type="button" class="row-play-btn" @click.stop="playChannel(channel)">
            <Play :size="13" fill="currentColor" />
          </button>
        </div>
      </div>
    </div>

    <!-- Playlist Chooser & Custom URL Modal / Popover -->
    <Transition name="fade-scale">
      <div v-if="showUrlModal" class="modal-backdrop" @click.self="showUrlModal = false">
        <div class="modal-card">
          <div class="modal-header">
            <div class="flex items-center gap-2">
              <Globe :size="18" class="text-purple-400" />
              <h3 class="modal-title">IPTV Playlists & Custom URLs</h3>
            </div>
            <button type="button" class="modal-close-btn" @click="showUrlModal = false">
              &times;
            </button>
          </div>

          <div class="modal-body">
            <!-- Add Custom URL Form -->
            <div class="add-url-section">
              <div class="section-subtitle">Add New M3U Playlist</div>
              <div class="flex flex-col gap-2 mt-2">
                <input v-model="newPlaylistName" type="text" placeholder="Playlist Name (e.g. My Favorite Channels)"
                  class="modal-input" />
                <div class="flex gap-2">
                  <input v-model="newPlaylistUrl" type="url" placeholder="https://example.com/playlist.m3u"
                    class="modal-input flex-1" @keyup.enter="saveCustomPlaylist" />
                  <button type="button" class="btn-modal-add" @click="saveCustomPlaylist">
                    <Plus :size="14" />
                    <span>Save & Load</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Saved Playlists List -->
            <div class="playlists-list-section mt-4">
              <div class="section-subtitle">Your Saved Playlists</div>
              <div class="saved-playlists-container">
                <div v-for="pl in savedPlaylists" :key="pl.id" class="saved-pl-item"
                  :class="{ active: currentUrl === pl.url }">
                  <div class="flex-1 min-w-0 cursor-pointer" @click="loadPlaylist(pl.url, true); showUrlModal = false">
                    <div class="flex items-center gap-2">
                      <span class="pl-name">{{ pl.name }}</span>
                      <Check v-if="currentUrl === pl.url" :size="14" class="text-purple-400" />
                    </div>
                    <div class="pl-url truncate">{{ pl.url }}</div>
                  </div>

                  <button type="button" class="pl-delete-btn" title="Remove Playlist"
                    @click.stop="removePlaylist(pl.id)">
                    <Trash2 :size="14" />
                  </button>
                </div>

                <div v-if="!savedPlaylists.length" class="text-xs text-white/40 italic py-3 text-center">
                  No saved playlists yet. Paste an M3U playlist link above to add channels.
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.iptv-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--bg-base);
  padding: 1rem 1.25rem 0.5rem;
  gap: 0.85rem;
}

/* Header */
.iptv-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-shrink: 0;
}

.iptv-brand {
  display: flex;
  align-items: center;
  gap: 0.85rem;
}

.tv-icon-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.75rem;
  height: 2.75rem;
  border-radius: 0.75rem;
  background: linear-gradient(135deg, #ef4444, #f59e0b);
  color: #fff;
  box-shadow: 0 4px 14px rgba(239, 68, 68, 0.35);
  flex-shrink: 0;
}

.iptv-title {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-primary, #ffffff);
  line-height: 1.2;
}

.live-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.1rem 0.4rem;
  border-radius: 9999px;
  background: rgba(239, 68, 68, 0.2);
  border: 1px solid rgba(239, 68, 68, 0.4);
  color: #f87171;
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.05em;
}

.live-pulse {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #ef4444;
  animation: pulse 1.6s infinite;
}

@keyframes pulse {
  0% {
    transform: scale(0.9);
    opacity: 0.8;
  }

  50% {
    transform: scale(1.3);
    opacity: 1;
  }

  100% {
    transform: scale(0.9);
    opacity: 0.8;
  }
}

.iptv-subtitle {
  margin: 0.2rem 0 0;
  font-size: 0.72rem;
  color: var(--text-muted, rgba(255, 255, 255, 0.6));
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.btn-iptv-action {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.75rem;
  border-radius: 0.5rem;
  background: var(--bg-elevated, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.12));
  color: var(--text-primary, #ffffff);
  font-size: 0.78rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-iptv-action:hover {
  background: var(--bg-hover, rgba(255, 255, 255, 0.12));
  border-color: rgba(255, 255, 255, 0.22);
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 0.5rem;
  background: var(--bg-elevated, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.12));
  color: var(--text-secondary, rgba(255, 255, 255, 0.8));
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-icon:hover {
  color: #fff;
  background: var(--bg-hover, rgba(255, 255, 255, 0.12));
}

.view-toggle {
  display: flex;
  background: var(--bg-elevated, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--border-strong, rgba(255, 255, 255, 0.12));
  border-radius: 0.5rem;
  padding: 0.15rem;
}

.view-toggle-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 0.35rem;
  color: var(--text-muted, rgba(255, 255, 255, 0.6));
  background: transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.view-toggle-btn.active {
  background: var(--accent-purple, #8b5cf6);
  color: #ffffff;
}

/* Toolbar */
.iptv-toolbar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
  background: var(--bg-panel, rgba(255, 255, 255, 0.03));
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
  border-radius: 0.65rem;
  padding: 0.45rem 0.65rem;
}

.search-box {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  max-width: 280px;
}

.search-icon {
  position: absolute;
  left: 0.6rem;
  color: var(--text-muted, rgba(255, 255, 255, 0.5));
  pointer-events: none;
}

.search-input {
  width: 100%;
  height: 1.95rem;
  padding: 0 1.8rem 0 2rem;
  border-radius: 0.45rem;
  background: rgba(0, 0, 0, 0.35);
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.1));
  color: #fff;
  font-size: 0.78rem;
  outline: none;
  transition: border-color 0.15s ease;
}

.search-input:focus {
  border-color: var(--accent-purple, #8b5cf6);
}

.clear-search-btn {
  position: absolute;
  right: 0.5rem;
  color: rgba(255, 255, 255, 0.6);
  font-size: 1rem;
  cursor: pointer;
  background: transparent;
}

.filter-select-wrapper {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.filter-label {
  font-size: 0.72rem;
  color: var(--text-muted, rgba(255, 255, 255, 0.6));
  font-weight: 500;
}

.filter-select {
  height: 1.95rem;
  padding: 0 0.6rem;
  border-radius: 0.45rem;
  background: rgba(0, 0, 0, 0.35);
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.1));
  color: var(--text-primary, #ffffff);
  font-size: 0.76rem;
  outline: none;
  cursor: pointer;
}

.filter-select option {
  background: #111827;
  color: #fff;
}

.channel-count-badge {
  margin-left: auto;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-muted, rgba(255, 255, 255, 0.6));
  font-variant-numeric: tabular-nums;
}

/* Content Area */
.iptv-content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: 1rem;
}

/* Empty / Loading State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 280px;
  width: 100%;
}

/* Channels Grid */
.channels-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 0.85rem;
}

.channel-card {
  display: flex;
  flex-direction: column;
  border-radius: 0.65rem;
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  box-shadow: var(--shadow-panel);
  overflow: hidden;
  cursor: pointer;
  transition: transform 0.15s ease, border-color 0.15s ease, background 0.15s ease;
}

.channel-card:hover {
  transform: translateY(-2px);
  border-color: var(--accent-purple, #8b5cf6);
  background: var(--bg-hover);
}

.card-logo-box {
  position: relative;
  height: 100px;
  background: var(--bg-elevated);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.75rem;
  overflow: hidden;
}

.channel-logo {
  max-height: 100%;
  max-width: 100%;
  object-fit: contain;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.5));
  z-index: 2;
}

.logo-fallback {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted, rgba(255, 255, 255, 0.3));
}

.card-play-overlay {
  position: absolute;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.15s ease;
  z-index: 4;
}

.channel-card:hover .card-play-overlay {
  opacity: 1;
}

.play-hero-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.25rem;
  height: 2.25rem;
  border-radius: 9999px;
  background: var(--accent-purple, #8b5cf6);
  box-shadow: 0 4px 12px rgba(139, 92, 246, 0.6);
  transform: scale(0.9);
  transition: transform 0.15s ease;
}

.channel-card:hover .play-hero-btn {
  transform: scale(1);
}

.card-meta {
  padding: 0.65rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.channel-name {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.channel-tags {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  overflow: hidden;
}

.tag-category {
  font-size: 9.5px;
  padding: 0.1rem 0.35rem;
  border-radius: 0.25rem;
  background: rgba(139, 92, 246, 0.18);
  color: #c4b5fd;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tag-lan {
  font-size: 9.5px;
  padding: 0.1rem 0.35rem;
  border-radius: 0.25rem;
  background: rgba(16, 185, 129, 0.18);
  color: #6ee7b7;
  white-space: nowrap;
}

/* Channels List */
.channels-list {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.list-row {
  display: grid;
  grid-template-columns: 36px 42px 1fr 140px 100px 36px;
  align-items: center;
  gap: 0.75rem;
  padding: 0.45rem 0.75rem;
  border-radius: 0.45rem;
  background: var(--bg-panel, rgba(255, 255, 255, 0.02));
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.15s ease;
}

.list-row:hover {
  background: var(--bg-hover, rgba(255, 255, 255, 0.07));
  border-color: rgba(139, 92, 246, 0.4);
}

.row-num {
  font-size: 0.72rem;
  color: var(--text-muted, rgba(255, 255, 255, 0.4));
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.row-logo-box {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 28px;
  border-radius: 0.35rem;
  background: rgba(0, 0, 0, 0.4);
  padding: 0.2rem;
  overflow: hidden;
}

.row-logo {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.row-title {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--text-primary, #ffffff);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-category {
  font-size: 0.72rem;
  color: #c4b5fd;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-lan {
  font-size: 0.72rem;
  color: #6ee7b7;
  white-space: nowrap;
}

.row-play-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.6rem;
  height: 1.6rem;
  border-radius: 50%;
  background: transparent;
  color: var(--text-secondary, rgba(255, 255, 255, 0.8));
  cursor: pointer;
  transition: all 0.15s ease;
}

.list-row:hover .row-play-btn {
  background: var(--accent-purple, #8b5cf6);
  color: #ffffff;
}

/* Modal / Dropdown Dialog */
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 80;
  background: rgba(0, 0, 0, 0.65);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
}

.modal-card {
  width: 100%;
  max-width: 540px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: 0.85rem;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.85rem 1.15rem;
  border-bottom: 1px solid var(--border-subtle);
}

.modal-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-primary);
}

.modal-close-btn {
  font-size: 1.25rem;
  color: var(--text-muted);
  background: transparent;
  cursor: pointer;
}

.modal-body {
  padding: 1rem 1.15rem;
  max-height: 70vh;
  overflow-y: auto;
}

.section-subtitle {
  font-size: 0.76rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--text-muted);
}

.modal-input {
  height: 2.15rem;
  padding: 0 0.75rem;
  border-radius: 0.45rem;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 0.78rem;
  outline: none;
}

.modal-input:focus {
  border-color: #8b5cf6;
}

.btn-modal-add {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0 0.85rem;
  height: 2.15rem;
  border-radius: 0.45rem;
  background: #8b5cf6;
  color: #fff;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
}

.saved-playlists-container {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: 0.45rem;
  max-height: 180px;
  overflow-y: auto;
}

.saved-pl-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.55rem 0.75rem;
  border-radius: 0.45rem;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
}

.saved-pl-item.active {
  border-color: #8b5cf6;
  background: rgba(139, 92, 246, 0.15);
}

.pl-name {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-primary);
}

.pl-url {
  font-size: 0.68rem;
  color: var(--text-muted);
}

.pl-delete-btn {
  color: rgba(255, 255, 255, 0.4);
  background: transparent;
  cursor: pointer;
  padding: 0.25rem;
}

.pl-delete-btn:hover {
  color: #ef4444;
}

.presets-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.preset-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.65rem;
  border-radius: 0.4rem;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.85);
  font-size: 0.74rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.preset-pill:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

.preset-pill.active {
  background: rgba(139, 92, 246, 0.3);
  border-color: #8b5cf6;
  color: #fff;
}
</style>
