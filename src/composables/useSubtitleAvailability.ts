import { ref } from "vue";
import { api } from "../services/tauri";
import type { MediaItem } from "../types/media";

// Shared across every list surface so a video fetched in one view is already
// known in the others. Availability comes from sidecar subtitle files on disk
// (srt/vtt/ass/ssa/sub), so it is queried once per item per session.
const available = ref<Record<number, boolean>>({});
const pending = new Map<number, Promise<boolean>>();

function get(id: number): boolean | undefined {
  return available.value[id];
}

function has(id: number): boolean {
  return available.value[id] === true;
}

/** Fetch availability for one video. Concurrent callers share the same request. */
function check(id: number): Promise<boolean> {
  if (id in available.value) return Promise.resolve(available.value[id]);
  const existing = pending.get(id);
  if (existing) return existing;

  const task = api.hasSubtitle(id)
    .then((value) => {
      available.value[id] = value;
      return value;
    })
    .catch(() => {
      available.value[id] = false;
      return false;
    })
    .finally(() => pending.delete(id));

  pending.set(id, task);
  return task;
}

/** Resolve a batch of videos with bounded concurrency to avoid flooding IPC. */
async function checkMany(items: MediaItem[], limit = 4): Promise<void> {
  const videos = items.filter((item) => item.kind === "video" && !(item.id in available.value));
  if (!videos.length) return;

  let cursor = 0;
  const worker = async () => {
    while (cursor < videos.length) {
      await check(videos[cursor++].id);
    }
  };
  await Promise.all(Array.from({ length: Math.min(limit, videos.length) }, worker));
}

/** Call after a subtitle is added/removed so the badge reflects the disk state. */
function invalidate(id: number): Promise<boolean> {
  const next = { ...available.value };
  delete next[id];
  available.value = next;
  return check(id);
}

export function useSubtitleAvailability() {
  return { available, get, has, check, checkMany, invalidate };
}
