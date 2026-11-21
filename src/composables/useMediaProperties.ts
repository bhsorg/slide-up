import { ref } from "vue";
import type { MediaItem } from "../types/media";

const open = ref(false);
const item = ref<MediaItem | null>(null);

function showProperties(media: MediaItem) {
  item.value = media;
  open.value = true;
}

function closeProperties() {
  open.value = false;
  item.value = null;
}

export function useMediaProperties() {
  return { open, item, showProperties, closeProperties };
}
