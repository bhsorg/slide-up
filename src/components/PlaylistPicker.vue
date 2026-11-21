<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ListMusic, Plus, X } from "@lucide/vue";
import { usePlaylistPicker } from "../composables/usePlaylistPicker";
import { usePlaylistStore } from "../stores/playlist";
import { useToast } from "../composables/useToast";

const { open, items, closePicker } = usePlaylistPicker();
const playlists = usePlaylistStore();
const toast = useToast();

const newName = ref("");
const busy = ref(false);
const count = computed(() => items.value.length);

async function performAdd(id: number, name: string) {
  const targets = [...items.value];
  try {
    for (const item of targets) {
      await playlists.addMedia(id, item);
    }
    toast.success(
      `Added ${targets.length} item${targets.length === 1 ? "" : "s"} to “${name}”.`,
    );
    newName.value = "";
    closePicker();
  } catch (error) {
    toast.error(String(error), "Could not add to playlist");
  } finally {
    busy.value = false;
  }
}

function addTo(id: number, name: string) {
  if (busy.value) return;
  busy.value = true;
  void performAdd(id, name);
}

async function createAndAdd() {
  const name = newName.value.trim();
  if (!name || busy.value) return;
  busy.value = true;
  try {
    const id = await playlists.create(name);
    await performAdd(id, name);
  } catch (error) {
    toast.error(String(error), "Could not create playlist");
    busy.value = false;
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closePicker();
}

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-[9998] flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
      @click.self="closePicker"
    >
      <div class="w-full max-w-sm overflow-hidden rounded-2xl border border-line-strong bg-elevated shadow-2xl">
        <div class="flex items-center justify-between border-b border-line px-5 py-4">
          <div>
            <h2 class="text-sm font-semibold text-ink">Add to playlist</h2>
            <p class="mt-0.5 text-[11px] text-faint">
              {{ count }} item{{ count === 1 ? "" : "s" }} selected
            </p>
          </div>
          <button
            type="button"
            class="flex h-7 w-7 items-center justify-center rounded-lg text-muted hover:bg-hover hover:text-ink"
            title="Close"
            @click="closePicker"
          >
            <X :size="16" />
          </button>
        </div>

        <div class="max-h-64 space-y-1 overflow-y-auto p-3 custom-scrollbar">
          <button
            v-for="playlist in playlists.playlists"
            :key="playlist.id"
            type="button"
            class="flex w-full items-center gap-2.5 rounded-lg border border-transparent px-3 py-2 text-left text-[12px] text-ink transition-colors hover:border-purple/40 hover:bg-purple/15"
            :disabled="busy"
            @click="addTo(playlist.id, playlist.name)"
          >
            <ListMusic :size="15" class="shrink-0 text-muted" />
            <span class="min-w-0 flex-1 truncate">{{ playlist.name }}</span>
            <span class="text-[10px] text-faint">{{ playlist.trackCount }}</span>
          </button>
          <p v-if="!playlists.playlists.length" class="px-3 py-4 text-center text-[11px] text-faint">
            No playlists yet. Create one below.
          </p>
        </div>

        <div class="border-t border-line p-3">
          <div class="flex gap-2">
            <input
              v-model="newName"
              type="text"
              placeholder="New playlist name"
              class="min-w-0 flex-1 rounded-lg border border-line bg-panel px-3 py-2 text-[12px] text-ink outline-none focus:border-purple/60"
              @keydown.enter="createAndAdd"
            />
            <button
              type="button"
              class="flex shrink-0 items-center gap-1 rounded-lg bg-purple px-3 py-2 text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
              :disabled="busy || !newName.trim()"
              @click="createAndAdd"
            >
              <Plus :size="14" />
              Create
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>
