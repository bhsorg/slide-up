import { reactive, readonly } from "vue";
import { events, onEvent } from "../services/tauri";

export type ToastKind = "info" | "success" | "error" | "warning";

export interface Toast {
  id: number;
  title: string;
  message: string;
  kind: ToastKind;
}

interface ToastInput {
  title?: string;
  message: string;
  kind?: ToastKind;
}

const state = reactive<{ toasts: Toast[] }>({ toasts: [] });
let nextId = 1;
let listening = false;

function dismiss(id: number) {
  const index = state.toasts.findIndex((toast) => toast.id === id);
  if (index !== -1) state.toasts.splice(index, 1);
}

function notify(input: ToastInput) {
  const toast: Toast = {
    id: nextId++,
    title: input.title ?? titleFor(input.kind ?? "info"),
    message: input.message,
    kind: input.kind ?? "info",
  };
  state.toasts.push(toast);
  if (state.toasts.length > 4) state.toasts.shift();
  window.setTimeout(() => dismiss(toast.id), 5200);
  return toast.id;
}

function titleFor(kind: ToastKind) {
  switch (kind) {
    case "success": return "Done";
    case "error": return "Something went wrong";
    case "warning": return "Heads up";
    default: return "Slide Up";
  }
}

function fromError(payload: unknown) {
  if (typeof payload === "string") {
    notify({ kind: "error", message: payload });
    return;
  }
  const error = payload as { title?: string; message?: string };
  notify({
    kind: "error",
    title: error.title,
    message: error.message ?? "An unexpected error occurred.",
  });
}

export function useToast() {
  function initialize() {
    if (listening) return;
    listening = true;
    void onEvent(events.toast, fromError);
    void onEvent<string>(events.playerError, (message) => {
      notify({ kind: "error", title: "Playback error", message });
    });
  }

  return {
    toasts: readonly(state.toasts),
    notify,
    dismiss,
    initialize,
    info: (message: string, title?: string) => notify({ message, title, kind: "info" }),
    success: (message: string, title?: string) => notify({ message, title, kind: "success" }),
    error: (message: string, title?: string) => notify({ message, title, kind: "error" }),
    warning: (message: string, title?: string) => notify({ message, title, kind: "warning" }),
  };
}
