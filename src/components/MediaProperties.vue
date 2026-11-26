<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { Clapperboard, X, User } from "@lucide/vue";
import { useMediaProperties } from "../composables/useMediaProperties";
import { useArtistBio } from "../composables/useArtistBio";
import { useLibraryStore } from "../stores/library";
import { formatBytes, formatDuration } from "../types/media";
import AppLogo from "./AppLogo.vue";

const { open, item, closeProperties } = useMediaProperties();
const { showArtist } = useArtistBio();
const library = useLibraryStore();

function openArtistBio(artistName: string) {
  closeProperties();
  void showArtist(artistName);
}

function formatDate(ms: number | null) {
  if (!ms) return "Never";
  return new Date(ms).toLocaleString();
}

const rows = computed(() => {
  const media = item.value;
  if (!media) return [];
  return [
    { label: "Type", value: media.kind === "video" ? "Video" : "Audio" },
    { label: "Artist", value: media.artist || "—" },
    { label: "Album", value: media.album || "—" },
    { label: "Duration", value: formatDuration(media.durationMs) },
    { label: "Codec", value: media.codec || "—" },
    {
      label: "Resolution",
      value:
        media.width != null && media.height != null && media.width > 0 && media.height > 0
          ? `${media.width} × ${media.height}`
          : "—",
    },
    { label: "File size", value: formatBytes(media.sizeBytes) },
    { label: "Favorite", value: media.favorite ? "Yes" : "No" },
    { label: "Play count", value: String(media.playCount) },
    { label: "Last played", value: formatDate(media.lastPlayedAt) },
    { label: "Added", value: formatDate(media.addedAt) },
    { label: "Media ID", value: String(media.id) },
  ];
});

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closeProperties();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open && item"
      class="fixed inset-0 z-[9998] flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
      @click.self="closeProperties"
    >
      <div class="w-full max-w-md overflow-hidden rounded-2xl border border-line-strong bg-elevated shadow-2xl">
        <div class="flex items-start gap-3 border-b border-line px-5 py-4">
          <div class="flex h-14 w-14 shrink-0 items-center justify-center overflow-hidden rounded-xl bg-panel text-muted">
            <img
              v-if="library.coverUrl(item)"
              :src="library.coverUrl(item)"
              :alt="item.title"
              class="h-full w-full object-cover"
            />
            <Clapperboard v-else-if="item.kind === 'video'" :size="24" />
            <AppLogo v-else class="h-10 w-10 opacity-80" />
          </div>
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-sm font-semibold text-ink">{{ item.title }}</h2>
            <p class="mt-0.5 text-[11px] text-faint">Media information</p>
          </div>
          <button
            type="button"
            class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted hover:bg-hover hover:text-ink"
            title="Close"
            @click="closeProperties"
          >
            <X :size="16" />
          </button>
        </div>

        <div class="max-h-[55vh] space-y-0.5 overflow-y-auto p-3 custom-scrollbar">
          <div
            v-for="row in rows"
            :key="row.label"
            class="flex items-start justify-between gap-4 rounded-lg px-3 py-1.5 text-[12px]"
          >
            <span class="shrink-0 text-faint">{{ row.label }}</span>
            <span v-if="row.label === 'Artist' && item.artist && !/unknown/i.test(item.artist)" class="min-w-0 flex items-center justify-end gap-1.5 text-right">
              <span class="break-all text-ink">{{ row.value }}</span>
              <button
                type="button"
                class="rounded bg-purple/15 px-1.5 py-0.5 text-[10.5px] font-medium text-purple-300 hover:bg-purple/25 transition-colors inline-flex items-center gap-1"
                title="View artist biopic & photo"
                @click="openArtistBio(item.artist)"
              >
                <User :size="10" /> Bio
              </button>
            </span>
            <span v-else class="min-w-0 break-all text-right text-ink">{{ row.value }}</span>
          </div>
          <div class="mt-2 border-t border-line px-3 pt-3">
            <p class="text-[11px] text-faint">File path</p>
            <p class="mt-1 break-all font-mono text-[11px] text-muted">{{ item.path }}</p>
          </div>
        </div>

        <div class="flex justify-end border-t border-line px-5 py-3">
          <button
            type="button"
            class="rounded-lg bg-purple px-4 py-1.5 text-[12px] font-medium text-white transition-opacity hover:opacity-90"
            @click="closeProperties"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
