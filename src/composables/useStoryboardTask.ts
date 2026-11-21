import { ref } from "vue";
import { api, events, onEvent } from "../services/tauri";

export interface StoryboardProgressPayload {
  mediaId: number;
  done: number;
  total: number;
  status: "running" | "done" | "error";
}

interface TaskState {
  running: boolean;
  done: number;
  total: number;
}

// Shared across every list surface so a generation started in one view is
// reflected in all of them.
const tasks = ref<Record<number, TaskState>>({});
const ready = ref<Record<number, boolean>>({});
let listening = false;

function ensureListening() {
  if (listening) return;
  listening = true;
  void onEvent<StoryboardProgressPayload>(events.storyboardProgress, (payload) => {
    if (payload.status === "running") {
      tasks.value[payload.mediaId] = {
        running: true,
        done: payload.done,
        total: payload.total,
      };
      return;
    }
    const next = { ...tasks.value };
    delete next[payload.mediaId];
    tasks.value = next;
    ready.value[payload.mediaId] = payload.status === "done";
  }).catch(() => {
    listening = false;
  });
}

function status(id: number): TaskState | undefined {
  return tasks.value[id];
}

function isReady(id: number): boolean {
  return ready.value[id] === true;
}

async function check(id: number) {
  ensureListening();
  if (id in ready.value) return;
  try {
    ready.value[id] = await api.hasStoryboard(id);
  } catch {
    ready.value[id] = false;
  }
}

async function generate(id: number) {
  ensureListening();
  tasks.value[id] = { running: true, done: 0, total: 12 };
  try {
    await api.generateStoryboard(id);
    ready.value[id] = true;
  } finally {
    const next = { ...tasks.value };
    delete next[id];
    tasks.value = next;
  }
}

export function useStoryboardTask() {
  ensureListening();
  return { tasks, ready, status, isReady, check, generate };
}
