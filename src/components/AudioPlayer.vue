<script setup lang="ts">
import { computed, watch } from "vue";
import {
  Play, Pause, SkipBack, SkipForward, Shuffle, Repeat, Repeat1, Heart,
  Volume2, VolumeX, ListPlus, Plus, MicVocal, User, Music2,
} from "@lucide/vue";
import { usePlayerStore } from "../stores/player";
import { useLibraryStore } from "../stores/library";
import { useToast } from "../composables/useToast";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { useLyrics } from "../composables/useLyrics";
import { useArtistBio } from "../composables/useArtistBio";
import AppLogo from "./AppLogo.vue";
import { formatDuration } from "../types/media";

const player = usePlayerStore();
const library = useLibraryStore();
const toast = useToast();
const picker = usePlaylistPicker();
const artistBio = useArtistBio();

const current = computed(() => player.state.current);
const artwork = computed(() => current.value?.thumbnailPath ? library.thumbnailUrl(current.value.thumbnailPath) : null);
const favorite = computed(() => (current.value ? (library.byId(current.value.id)?.favorite ?? false) : false));
const bars = Array.from({ length: 28 }, (_, index) => index);

const {
  status: lyricsStatus,
  activeLine,
  prevLine,
  nextLine,
  errorMessage: lyricsError,
  showLyrics,
  setShowLyrics,
  load: loadLyrics,
  clear: clearLyrics,
  updatePosition: updateLyricPosition,
} = useLyrics();

function refreshLyrics(force = false) {
  const track = player.state.current;
  if (!showLyrics.value || !track) return;
  void loadLyrics(track, force).then(() => updateLyricPosition(player.state.positionMs));
}

watch(() => player.state.current?.id, () => {
  if (!showLyrics.value) clearLyrics();
  else refreshLyrics();
}, { immediate: true });

watch(() => player.state.positionMs, (positionMs) => updateLyricPosition(positionMs));

function toggleLyrics() {
  setShowLyrics(!showLyrics.value);
  if (showLyrics.value) refreshLyrics();
}

function retryLyrics() {
  refreshLyrics(true);
}

function seekToLine(timeMs: number) {
  if (timeMs >= 0) void player.seek(timeMs);
}

async function toggleFavorite() {
  if (!current.value) return;
  const item = library.byId(current.value.id);
  if (!item) return;
  try {
    await library.toggleFavorite(item);
  } catch (error) {
    toast.error(String(error), "Could not update favorite");
  }
}

async function addToQueue() {
  if (!current.value) return;
  const item = library.byId(current.value.id);
  if (!item) return;
  try {
    await player.queueAdd(item);
    toast.success(`Queued “${item.title}”.`);
  } catch (error) {
    toast.error(String(error), "Could not queue item");
  }
}

function addToPlaylist() {
  if (!current.value) return;
  const item = library.byId(current.value.id);
  if (!item) return;
  picker.askAddToPlaylist(item);
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col items-center justify-center overflow-y-auto px-6 py-4">
    <template v-if="current">
      <div class="relative mb-5">
        <div class="absolute -inset-10 rounded-full bg-purple/25 blur-3xl" />
        <div class="relative h-[min(38vh,320px)] w-[min(38vh,320px)] overflow-hidden rounded-3xl border border-line-strong bg-elevated shadow-[var(--shadow-panel)]">
          <img v-if="artwork" :src="artwork" :alt="current.title" class="h-full w-full object-cover" />
          <div v-else class="flex h-full w-full items-center justify-center bg-linear-to-br from-purple/35 via-panel to-blue/30">
            <AppLogo class="h-2/3 w-2/3 opacity-80" />
          </div>
        </div>
      </div>

      <div class="text-center">
        <h1 class="truncate text-2xl font-bold tracking-tight text-ink">{{ current.title }}</h1>
        <div class="mt-1 flex items-center justify-center gap-2 truncate text-sm text-muted">
          <button
            type="button"
            class="hover:text-purple-300 hover:underline cursor-pointer transition-colors font-medium"
            title="View artist biopic & photo"
            @click="artistBio.showArtist(current.artist || '')"
          >
            {{ current.artist || "Unknown artist" }}
          </button>
          <span v-if="current.album" class="text-faint">· {{ current.album }}</span>
          <button
            v-if="current.artist && !/unknown/i.test(current.artist)"
            type="button"
            class="inline-flex items-center gap-1 rounded-full bg-purple/15 px-2 py-0.5 text-[10.5px] font-medium text-purple-300 hover:bg-purple/25 transition-colors"
            title="Artist bio & photo"
            @click="artistBio.showArtist(current.artist)"
          >
            <User :size="11" />
            <span>Bio</span>
          </button>
        </div>
      </div>

      <!-- Line-by-Line Lyrics Display (No textarea box, pure typography) -->
      <div v-if="showLyrics" class="mt-4 w-full max-w-[620px] px-4 text-center select-none">
        <div v-if="lyricsStatus === 'loading'" class="py-2 text-xs text-muted animate-pulse">
          Looking up lyrics…
        </div>

        <div v-else-if="lyricsStatus === 'notfound'" class="py-2 flex items-center justify-center gap-2 text-xs text-muted">
          <span>No lyrics found for this track.</span>
          <button
            type="button"
            class="text-purple underline hover:text-purple-300 font-medium ml-1"
            @click="retryLyrics"
          >Retry</button>
        </div>

        <div v-else-if="lyricsStatus === 'error'" class="py-2 flex items-center justify-center gap-2 text-xs text-muted">
          <span>{{ lyricsError }}</span>
          <button
            type="button"
            class="text-purple underline hover:text-purple-300 font-medium ml-1"
            @click="retryLyrics"
          >Retry</button>
        </div>

        <div v-else-if="lyricsStatus === 'ready'" class="flex flex-col items-center justify-center min-h-[68px]">
          <!-- Previous Line -->
          <div
            v-if="prevLine?.text"
            class="text-[12px] text-muted/40 transition-all duration-300 line-clamp-1 mb-1 cursor-pointer hover:text-muted/70"
            @click="seekToLine(prevLine.timeMs)"
          >
            {{ prevLine.text }}
          </div>

          <!-- Active Current Line (Prominent & Focused) -->
          <div
            class="text-lg sm:text-xl font-bold tracking-tight text-purple-soft transition-all duration-300 drop-shadow-sm px-2 cursor-pointer hover:text-white"
            @click="activeLine ? seekToLine(activeLine.timeMs) : undefined"
          >
            {{ activeLine?.text || "♫ ♪ ♫" }}
          </div>

          <!-- Next Line Preview -->
          <div
            v-if="nextLine?.text"
            class="text-[12px] text-muted/50 transition-all duration-300 line-clamp-1 mt-1 cursor-pointer hover:text-muted/80"
            @click="seekToLine(nextLine.timeMs)"
          >
            {{ nextLine.text }}
          </div>
        </div>
      </div>

      <div class="mt-4 flex h-8 items-end gap-[3px]" aria-hidden="true">
        <span
          v-for="bar in bars"
          :key="bar"
          class="w-[3px] rounded-full bg-linear-to-t from-purple to-blue transition-[height] duration-300"
          :class="player.isPlaying ? 'animate-pulse' : 'opacity-30'"
          :style="{ height: `${8 + ((bar * 37) % 24)}px`, animationDelay: `${(bar % 7) * 90}ms` }"
        />
      </div>

      <div class="mt-5 w-full max-w-[680px]">
        <div class="flex items-center gap-3">
          <span class="w-12 text-right text-[11px] tabular-nums text-muted">{{ formatDuration(player.state.positionMs) }}</span>
          <input
            class="su-range flex-1"
            type="range"
            min="0"
            :max="Math.max(player.state.durationMs, 1)"
            :value="player.state.positionMs"
            :style="{ '--fill': `${player.progress}%` }"
            aria-label="Seek"
            @input="player.seek(Number(($event.target as HTMLInputElement).value))"
          />
          <span class="w-12 text-[11px] tabular-nums text-muted">{{ formatDuration(player.state.durationMs) }}</span>
        </div>

        <div class="mt-4 flex items-center justify-center gap-3">
          <button type="button" class="audio-control" :class="player.state.shuffle ? 'text-purple' : ''" title="Shuffle" @click="player.setShuffle(!player.state.shuffle)"><Shuffle :size="18" /></button>
          <button type="button" class="audio-control" title="Previous track" @click="player.previous()"><SkipBack :size="22" fill="currentColor" /></button>
          <button
            type="button"
            class="flex h-14 w-14 items-center justify-center rounded-full bg-linear-to-br from-purple to-blue text-white shadow-[var(--glow-purple)] transition-transform hover:scale-105"
            title="Play / Pause"
            @click="player.togglePause()"
          >
            <Pause v-if="player.isPlaying" :size="22" fill="currentColor" />
            <Play v-else :size="22" fill="currentColor" />
          </button>
          <button type="button" class="audio-control" title="Next track" @click="player.next()"><SkipForward :size="22" fill="currentColor" /></button>
          <button
            type="button"
            class="audio-control"
            :class="player.state.repeat !== 'off' ? 'text-purple' : ''"
            :title="`Repeat: ${player.state.repeat}`"
            @click="player.setRepeat(player.state.repeat === 'off' ? 'all' : player.state.repeat === 'all' ? 'one' : 'off')"
          >
            <Repeat1 v-if="player.state.repeat === 'one'" :size="18" />
            <Repeat v-else :size="18" />
          </button>
        </div>

        <div class="mt-4 flex items-center gap-3">
          <button type="button" class="audio-control" :class="favorite ? 'text-[#ff5b8d]' : ''" title="Favorite" @click="toggleFavorite">
            <Heart :size="17" :fill="favorite ? 'currentColor' : 'none'" />
          </button>
          <button type="button" class="audio-control" title="Add to queue" @click="addToQueue"><ListPlus :size="17" /></button>
          <button type="button" class="audio-control" title="Add to playlist…" @click="addToPlaylist"><Plus :size="17" /></button>
          <button
            type="button"
            class="audio-control"
            :class="showLyrics ? 'text-purple' : ''"
            :title="showLyrics ? 'Hide lyrics' : 'Show lyrics (LRCLIB)'"
            @click="toggleLyrics"
          ><MicVocal :size="17" /></button>

          <div class="ml-auto flex items-center gap-2">
            <button type="button" class="audio-control" title="Mute" @click="player.toggleMute()">
              <VolumeX v-if="player.state.muted || player.state.volume === 0" :size="17" />
              <Volume2 v-else :size="17" />
            </button>
            <input
              class="su-range w-28"
              type="range"
              min="0"
              max="1"
              step="0.01"
              :value="player.state.muted ? 0 : player.state.volume"
              :style="{ '--fill': `${(player.state.muted ? 0 : player.state.volume) * 100}%` }"
              aria-label="Volume"
              @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
            />
            <select
              class="h-8 rounded-lg border border-line bg-elevated px-2 text-[11px] text-muted outline-none"
              title="Playback speed"
              :value="player.state.speed"
              @change="player.setSpeed(Number(($event.target as HTMLSelectElement).value))"
            >
              <option v-for="speed in [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 4]" :key="speed" :value="speed">{{ speed }}×</option>
            </select>
          </div>
        </div>
      </div>
    </template>

    <div v-else class="flex flex-col items-center text-center">
      <div class="mb-4 flex h-20 w-20 items-center justify-center rounded-3xl bg-elevated text-faint"><Music2 :size="34" /></div>
      <h2 class="text-lg font-semibold text-ink">No track playing</h2>
      <p class="mt-1 text-sm text-muted">Pick a song from your music library to start listening.</p>
    </div>
  </div>
</template>

<style scoped>
.audio-control {
  display: flex;
  height: 2.25rem;
  width: 2.25rem;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  color: var(--text-secondary);
  transition: background var(--t-fast), color var(--t-fast);
}
.audio-control:hover { background: var(--bg-hover); color: var(--text-primary); }

.lyrics-viewport::-webkit-scrollbar { width: 5px; }
.lyrics-viewport::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.18); border-radius: 999px; }
</style>
