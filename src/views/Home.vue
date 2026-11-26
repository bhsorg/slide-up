<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Play, FolderPlus, FilePlus2, Music2, Clapperboard, Heart, Sparkles, Globe, X, ListPlus, LayoutGrid, List } from "@lucide/vue";
import { useLibraryStore } from "../stores/library";
import { usePlayerStore } from "../stores/player";
import { usePlaylistStore } from "../stores/playlist";
import { useToast } from "../composables/useToast";
import MediaGrid from "../components/MediaGrid.vue";
import AppLogo from "../components/AppLogo.vue";
import type { MediaItem } from "../types/media";
import type { ViewId } from "../types/settings";

const emit = defineEmits<{ navigate: [view: ViewId | "player"] }>();

const library = useLibraryStore();
const player = usePlayerStore();
const playlists = usePlaylistStore();
const toast = useToast();

const viewMode = ref<"grid" | "list">(
  (localStorage.getItem("slideup-home-view") as "grid" | "list" | null) ?? "grid",
);
watch(viewMode, (value) => localStorage.setItem("slideup-home-view", value));

const showStreamModal = ref(false);
const streamUrl = ref("");
const streamTitle = ref("");

const featured = computed(() => library.recent[0] ?? library.items[0] ?? null);
const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
});

function play(items: MediaItem[], index: number) {
  void player.playList(items, index, "Library").catch((error: unknown) => toast.error(String(error), "Playback failed"));
}

async function addFiles() {
  try {
    const result = await library.importFiles();
    if (result) toast.success(`Imported ${result.added} item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Import failed");
  }
}

async function addFolder() {
  try {
    const result = await library.scanFolder();
    if (result) toast.success(`Scanned folder · ${result.added} new item${result.added === 1 ? "" : "s"}.`);
  } catch (error) {
    toast.error(String(error), "Scan failed");
  }
}

async function playStream(queueOnly = false) {
  const url = streamUrl.value.trim();
  if (!url) {
    toast.error("Please enter a stream URL");
    return;
  }
  if (!/^https?:\/\/|^rt[ms]p:\/\//i.test(url)) {
    toast.error("URL must begin with http://, https://, rtmp://, or rtsp://");
    return;
  }

  // If user pasted an .m3u playlist link, parse & import it rather than sending raw text file to MPV
  if (/\.m3u(\?.*)?$/i.test(url)) {
    try {
      toast.info("Parsing M3U playlist tracks…");
      const res = await playlists.importM3uFromUrl(url, streamTitle.value);
      toast.success(`Imported playlist “${res.name}” (${res.total} tracks · ${res.audioCount} Audio, ${res.videoCount} Video).`);
      if (!queueOnly) {
        await playlists.play(res.id, 0);
      }
      streamUrl.value = "";
      streamTitle.value = "";
      showStreamModal.value = false;
      return;
    } catch (err) {
      toast.error(String(err), "Could not import M3U playlist");
      return;
    }
  }

  const rawFilename = url.split("/").pop()?.split("?")[0] || "Network Stream";
  let decodedFilename = rawFilename;
  try {
    decodedFilename = decodeURIComponent(rawFilename);
  } catch {
    // keep raw if URI is not percent-encoded
  }
  const name = streamTitle.value.trim() || decodedFilename;
  const isAudioUrl = /\.(mp3|aac|m4a|flac|ogg|opus|wav|wma)(\?.*)?$/i.test(url) || /radio|podcast|audio/i.test(url);
  const streamItem: MediaItem = {
    id: -Math.floor(Date.now() % 1000000000),
    path: url,
    kind: isAudioUrl ? "audio" : "video",
    title: name,
    artist: isAudioUrl ? "Audio Stream" : "Network Stream",
    album: "Stream",
    durationMs: 0,
    codec: null,
    width: null,
    height: null,
    sizeBytes: 0,
    thumbnailPath: null,
    favorite: false,
    playCount: 0,
    lastPlayedAt: Date.now(),
    addedAt: Date.now(),
  };

  try {
    if (queueOnly) {
      await player.queueAdd(streamItem);
      toast.success(`Added “${name}” to queue.`);
    } else {
      await player.play(streamItem);
      emit("navigate", "player");
    }
    streamUrl.value = "";
    streamTitle.value = "";
    showStreamModal.value = false;
  } catch (err) {
    toast.error(String(err), "Stream playback failed");
  }
}
</script>

<template>
  <div class="h-full overflow-y-auto px-5 py-4 custom-scrollbar">
    <div class="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div>
        <div class="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-purple-soft">
          <Sparkles :size="13" /> Your space
        </div>
        <h1 class="text-xl font-bold tracking-tight text-ink">{{ greeting }} 👋</h1>
        <p class="mt-0.5 text-[12px] text-muted">Everything you love to watch and listen to, in one place.</p>
      </div>
      <div class="flex items-center gap-2">
        <div class="flex items-center rounded-xl border border-line bg-panel p-1">
          <button
            type="button"
            class="view-toggle"
            :class="viewMode === 'grid' ? 'bg-purple text-white' : 'text-faint'"
            title="Grid view"
            @click="viewMode = 'grid'"
          >
            <LayoutGrid :size="15" />
          </button>
          <button
            type="button"
            class="view-toggle"
            :class="viewMode === 'list' ? 'bg-purple text-white' : 'text-faint'"
            title="List view"
            @click="viewMode = 'list'"
          >
            <List :size="15" />
          </button>
        </div>
        <button type="button" class="home-action" @click="addFiles"><FilePlus2 :size="14" /> Add files</button>
        <button type="button" class="home-action" @click="addFolder"><FolderPlus :size="14" /> Add folder</button>
        <button type="button" class="home-action" @click="showStreamModal = true"><Globe :size="14" /> Add stream</button>
      </div>
    </div>

    <div class="mb-5 grid grid-cols-3 gap-3">
      <button type="button" class="quick-card group" @click="emit('navigate', 'music')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-purple to-blue text-white"><Music2 :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Music</span>
          <span class="block text-[10.5px] text-faint">{{ library.audioItems.length }} tracks</span>
        </span>
      </button>
      <button type="button" class="quick-card group" @click="emit('navigate', 'videos')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-blue to-purple text-white"><Clapperboard :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Videos</span>
          <span class="block text-[10.5px] text-faint">{{ library.videoItems.length }} videos</span>
        </span>
      </button>
      <button type="button" class="quick-card group" @click="emit('navigate', 'favorites')">
        <span class="flex h-9 w-9 items-center justify-center rounded-lg bg-linear-to-br from-[#ff4d8d] to-[#ff9a5c] text-white"><Heart :size="17" /></span>
        <span class="text-left">
          <span class="block text-[12.5px] font-semibold text-ink">Favorites</span>
          <span class="block text-[10.5px] text-faint">{{ library.favorites.length }} liked</span>
        </span>
      </button>
    </div>

    <div v-if="featured" class="su-panel mb-5 flex flex-wrap items-center gap-4 overflow-hidden p-4">
      <div class="relative h-24 w-38 shrink-0 overflow-hidden rounded-lg bg-elevated">
        <img v-if="library.coverUrl(featured)" :src="library.coverUrl(featured)" :alt="featured.title" class="h-full w-full object-cover" />
        <div v-else class="flex h-full w-full items-center justify-center bg-linear-to-br from-purple/25 to-blue/25 text-white/30">
          <Clapperboard v-if="featured.kind === 'video'" :size="24" />
          <AppLogo v-else class="h-14 w-14 opacity-80" />
        </div>
      </div>
      <div class="min-w-0 flex-1">
        <div class="mb-1 text-[10px] font-semibold uppercase tracking-wider text-purple-soft">Continue where you left off</div>
        <h2 class="truncate text-lg font-bold text-ink">{{ featured.title }}</h2>
        <p class="mt-0.5 truncate text-[11px] text-muted">{{ featured.artist || "Your library" }} · {{ featured.kind === 'video' ? 'Video' : 'Audio' }}</p>
      </div>
      <button type="button" class="flex h-9.5 items-center gap-2 rounded-lg bg-purple px-4 text-[12.5px] font-semibold text-white hover:opacity-90 transition-opacity" @click="player.play(featured)">
        <Play :size="15" fill="currentColor" /> Play now
      </button>
    </div>

    <MediaGrid
      :items="library.recent.slice(0, 12)"
      :mode="viewMode"
      :selectable="false"
      context-name="Recently played"
      @play="play"
    />

    <!-- Open Network Stream Modal -->
    <Transition name="fade-scale">
      <div v-if="showStreamModal" class="modal-backdrop" @click.self="showStreamModal = false">
        <div class="modal-card">
          <div class="modal-header">
            <div class="flex items-center gap-2">
              <Globe :size="17" class="text-purple-400" />
              <h3 class="modal-title">Open Network Stream</h3>
            </div>
            <button type="button" class="modal-close-btn" @click="showStreamModal = false">
              <X :size="16" />
            </button>
          </div>

          <div class="modal-body space-y-3">
            <div>
              <label class="block text-[11px] font-semibold text-muted uppercase tracking-wider mb-1.5">
                Stream / Media URL
              </label>
              <input
                v-model="streamUrl"
                type="url"
                placeholder="https://example.com/live/stream.m3u8 or media URL"
                class="stream-input"
                autofocus
                @keyup.enter="playStream(false)"
              />
              <p class="mt-1 text-[10.5px] text-faint">
                Supports HLS (.m3u8), RTSP, RTMP, HTTP/HTTPS audio & video streams.
              </p>
            </div>

            <div>
              <label class="block text-[11px] font-semibold text-muted uppercase tracking-wider mb-1.5">
                Stream Title (Optional)
              </label>
              <input
                v-model="streamTitle"
                type="text"
                placeholder="e.g. My Online Video / Live Radio"
                class="stream-input"
                @keyup.enter="playStream(false)"
              />
            </div>
          </div>

          <div class="modal-footer">
            <button
              type="button"
              class="btn-footer-secondary"
              @click="playStream(true)"
            >
              <ListPlus :size="14" />
              <span>Add to Queue</span>
            </button>

            <button
              type="button"
              class="btn-footer-primary"
              @click="playStream(false)"
            >
              <Play :size="14" fill="currentColor" />
              <span>Play Now</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.quick-card {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  border-radius: var(--radius-md, 12px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0.85rem;
  transition: border-color var(--t-fast), background var(--t-fast);
}
.quick-card:hover {
  border-color: var(--border-strong);
  background: var(--bg-hover);
}
.home-action {
  display: flex;
  height: 2.1rem;
  align-items: center;
  gap: 0.4rem;
  border-radius: var(--radius-sm, 8px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0 0.75rem;
  font-size: 0.72rem;
  font-weight: 500;
  color: var(--text-secondary);
  transition: color var(--t-fast), border-color var(--t-fast);
}
.home-action:hover {
  color: var(--text-primary);
  border-color: var(--border-strong);
}

.view-toggle {
  display: flex;
  height: 1.85rem;
  width: 1.85rem;
  align-items: center;
  justify-content: center;
  border-radius: 0.5rem;
  transition: background var(--t-fast), color var(--t-fast);
}
.view-toggle:hover { color: var(--text-primary); background: var(--bg-hover); }

/* Stream Modal Styles */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 1rem;
}

.modal-card {
  width: 100%;
  max-width: 460px;
  background: var(--bg-panel);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md, 12px);
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
  display: flex;
  align-items: center;
  justify-content: center;
  height: 1.75rem;
  width: 1.75rem;
  border-radius: 6px;
  color: var(--text-muted);
  cursor: pointer;
  transition: background var(--t-fast), color var(--t-fast);
}

.modal-close-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.modal-body {
  padding: 1.15rem;
}

.stream-input {
  width: 100%;
  height: 2.25rem;
  padding: 0 0.75rem;
  border-radius: 8px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 0.82rem;
  outline: none;
  transition: border-color var(--t-fast);
}

.stream-input:focus {
  border-color: var(--accent-purple, #8b5cf6);
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 0.85rem 1.15rem;
  border-top: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
}

.btn-footer-secondary {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  height: 2.15rem;
  padding: 0 0.85rem;
  border-radius: 8px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  color: var(--text-primary);
  font-size: 0.78rem;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--t-fast);
}

.btn-footer-secondary:hover {
  background: var(--bg-hover);
  border-color: var(--border-strong);
}

.btn-footer-primary {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  height: 2.15rem;
  padding: 0 1rem;
  border-radius: 8px;
  background: var(--accent-purple, #8b5cf6);
  color: #ffffff;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--t-fast);
  box-shadow: 0 2px 8px rgba(139, 92, 246, 0.4);
}

.btn-footer-primary:hover {
  opacity: 0.92;
  transform: translateY(-1px);
}

.fade-scale-enter-active,
.fade-scale-leave-active {
  transition: all 0.15s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-scale-enter-from,
.fade-scale-leave-to {
  opacity: 0;
  transform: scale(0.96);
}
</style>