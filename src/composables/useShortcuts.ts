import { usePlayerStore } from "../stores/player";
import { useSettingsStore } from "../stores/settings";
import { useLibraryStore } from "../stores/library";
import { useToast } from "./useToast";
import type { Shortcuts } from "../types/settings";

type Action = keyof Shortcuts;

let listening = false;
let detach: (() => void) | null = null;

function comboFromEvent(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.shiftKey) parts.push("Shift");
  if (event.altKey) parts.push("Alt");
  if (event.metaKey) parts.push("Meta");
  const raw = event.key;
  const key =
    raw === " "
      ? "Space"
      : raw.length === 1 && parts.length === 0
        ? raw.toLowerCase()
        : raw.length === 1
          ? raw.toUpperCase()
          : raw;
  parts.push(key);
  return parts.join("+");
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
}

export function useShortcuts() {
  const player = usePlayerStore();
  const settings = useSettingsStore();
  const library = useLibraryStore();
  const toast = useToast();

  function handle(event: KeyboardEvent) {
    if (isTypingTarget(event.target)) return;
    if (event.repeat && event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;

    const combo = comboFromEvent(event);
    const map = new Map<string, Action>();
    for (const [action, value] of Object.entries(settings.shortcuts)) {
      if (value) map.set(value.toLowerCase(), action as Action);
    }
    const action = map.get(combo.toLowerCase());
    if (!action) return;

    switch (action) {
      case "playPause":
        void player.togglePause();
        break;
      case "seekBack":
        void player.seekRelative(-5_000);
        break;
      case "seekForward":
        void player.seekRelative(5_000);
        break;
      case "volumeUp":
        void player.setVolume(Math.min(1, player.state.volume + 0.05));
        break;
      case "volumeDown":
        void player.setVolume(Math.max(0, player.state.volume - 0.05));
        break;
      case "fullscreen":
        void player.setFullscreen(!player.state.fullscreen);
        break;
      case "exitFullscreen":
        if (player.state.fullscreen) void player.setFullscreen(false);
        return;
      case "mute":
        void player.toggleMute();
        break;
      case "next":
        void player.next();
        break;
      case "previous":
        void player.previous();
        break;
      case "openFile":
        void library.importFiles().then((result) => {
          if (result && result.added > 0) toast.success(`Added ${result.added} item${result.added === 1 ? "" : "s"} to your library.`);
        }).catch((error: unknown) => toast.error(String(error)));
        break;
      case "openFolder":
        void library.scanFolder().then((result) => {
          if (result && result.added > 0) toast.success(`Added ${result.added} item${result.added === 1 ? "" : "s"} from the folder.`);
        }).catch((error: unknown) => toast.error(String(error)));
        break;
    }
    event.preventDefault();
  }

  function initialize() {
    if (listening) return;
    listening = true;
    window.addEventListener("keydown", handle);
    detach = () => window.removeEventListener("keydown", handle);
  }

  function dispose() {
    detach?.();
    detach = null;
    listening = false;
  }

  return { initialize, dispose };
}
