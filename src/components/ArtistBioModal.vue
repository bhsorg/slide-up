<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { User, X, ExternalLink, Sparkles } from "@lucide/vue";
import { useArtistBio } from "../composables/useArtistBio";
import { api } from "../services/tauri";

const { open, loading, currentArtist, bio, error, closeArtist } = useArtistBio();

function openWikipedia() {
  if (bio.value?.wikipediaUrl) {
    void api.openExternal(bio.value.wikipediaUrl);
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closeArtist();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div
        v-if="open"
        class="fixed inset-0 z-[9998] flex items-center justify-center bg-black/65 p-4 backdrop-blur-sm"
        @click.self="closeArtist"
      >
        <div class="relative flex max-h-[85vh] w-full max-w-lg flex-col overflow-hidden rounded-2xl border border-line-strong bg-elevated shadow-2xl">
          <!-- Modal Header -->
          <div class="flex items-center justify-between border-b border-line px-5 py-4">
            <div class="flex items-center gap-2.5 min-w-0">
              <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-purple/15 text-purple-400">
                <User :size="17" />
              </span>
              <div class="min-w-0">
                <h3 class="truncate text-[15px] font-bold text-ink">
                  {{ bio?.name || currentArtist }}
                </h3>
                <p v-if="bio?.description" class="truncate text-[11px] text-muted">
                  {{ bio.description }}
                </p>
              </div>
            </div>
            <button
              type="button"
              class="flex h-8 w-8 items-center justify-center rounded-lg text-faint hover:bg-panel hover:text-ink transition-colors ml-2"
              @click="closeArtist"
            >
              <X :size="17" />
            </button>
          </div>

          <!-- Modal Body -->
          <div class="flex-1 overflow-y-auto p-5 custom-scrollbar">
            <!-- Loading State -->
            <div v-if="loading" class="flex flex-col items-center justify-center py-12 text-center">
              <div class="h-10 w-10 animate-spin rounded-full border-2 border-purple border-t-transparent mb-3" />
              <p class="text-xs font-medium text-muted">Fetching artist biography & photo…</p>
            </div>

            <!-- Error State -->
            <div v-else-if="error" class="flex flex-col items-center justify-center py-10 text-center">
              <div class="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-panel text-faint">
                <User :size="24" />
              </div>
              <p class="text-sm font-semibold text-ink">{{ currentArtist }}</p>
              <p class="mt-1 text-xs text-muted">{{ error }}</p>
            </div>

            <!-- Content State -->
            <div v-else-if="bio" class="space-y-4">
              <!-- Artist High-Res Photo -->
              <div v-if="bio.photoUrl" class="relative overflow-hidden rounded-xl bg-panel max-h-[260px] flex items-center justify-center border border-line">
                <img
                  :src="bio.photoUrl"
                  :alt="bio.name"
                  class="h-full w-full object-cover object-top max-h-[260px]"
                  loading="lazy"
                />
                <div class="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent pointer-events-none" />
                <span class="absolute bottom-2.5 left-3 text-[11px] font-semibold text-white/90 drop-shadow">
                  {{ bio.name }}
                </span>
              </div>

              <!-- Biography Text -->
              <div>
                <div class="mb-1.5 flex items-center gap-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-purple-soft">
                  <Sparkles :size="12" /> Biography
                </div>
                <p class="text-[13px] leading-relaxed text-ink/90 whitespace-pre-line text-justify">
                  {{ bio.biography }}
                </p>
              </div>
            </div>
          </div>

          <!-- Modal Footer -->
          <div v-if="bio?.wikipediaUrl" class="flex items-center justify-between border-t border-line bg-panel/50 px-5 py-3">
            <span class="text-[11px] text-faint">Source: Wikipedia</span>
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-lg bg-purple/15 px-3 py-1.5 text-[11.5px] font-medium text-purple-300 hover:bg-purple/25 transition-colors"
              @click="openWikipedia"
            >
              <span>Read more on Wikipedia</span>
              <ExternalLink :size="13" />
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
