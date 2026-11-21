import { ref } from "vue";
import type { MediaItem } from "../types/media";

// Singleton state for the global "Add to playlist" chooser. Any component can
// request a picker; the single PlaylistPicker dialog mounted in App.vue serves
// all of them.
const open = ref(false);
const items = ref<MediaItem[]>([]);

export function usePlaylistPicker() {
  function askAddToPlaylist(media: MediaItem | MediaItem[]) {
    items.value = Array.isArray(media) ? [...media] : [media];
    open.value = true;
  }

  function closePicker() {
    open.value = false;
    items.value = [];
  }

  return { open, items, askAddToPlaylist, closePicker };
}
