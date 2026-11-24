import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";

import "./styles/tokens.css";
import "./styles/base.css";

if (import.meta.env.DEV) {
  document.body.classList.add("dev-mode");
} else {
  // Production / Release: silence verbose console logs
  console.log = () => {};
  console.debug = () => {};
  console.info = () => {};
}

// Global protection: Block browser contextmenu & default web shortcuts
window.addEventListener("keydown", (e: KeyboardEvent) => {
  const isCtrlOrCmd = e.ctrlKey || e.metaKey;
  const key = e.key.toLowerCase();
  const target = e.target as HTMLElement | null;
  const isInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

  // Block Ctrl+P (Print) and Ctrl+S (Save Page)
  if (isCtrlOrCmd && (key === "p" || key === "s")) {
    e.preventDefault();
    return;
  }

  // Block Ctrl+F (Browser Find) and redirect to App Search Bar
  if (isCtrlOrCmd && key === "f") {
    e.preventDefault();
    const searchInput = document.querySelector<HTMLInputElement>('header input[type="search"]');
    if (searchInput) {
      searchInput.focus();
      searchInput.select();
    }
    return;
  }

  // Release mode restrictions
  if (!import.meta.env.DEV) {
    // Block Zoom shortcuts: Ctrl + '=', Ctrl + '+', Ctrl + '-', Ctrl + '0'
    if (isCtrlOrCmd && (key === "=" || key === "+" || key === "-" || key === "_" || key === "0")) {
      e.preventDefault();
      return;
    }

    // Block F5 and Ctrl+R (Reload app page)
    if (e.key === "F5" || (isCtrlOrCmd && key === "r")) {
      e.preventDefault();
      return;
    }

    // Block F12, Ctrl+Shift+I / J (DevTools inspect)
    if (
      e.key === "F12" ||
      (isCtrlOrCmd && e.shiftKey && (key === "i" || key === "j"))
    ) {
      e.preventDefault();
      return;
    }

    // Block Ctrl+U (View Source)
    if (isCtrlOrCmd && key === "u") {
      e.preventDefault();
      return;
    }

    // Block Alt + ArrowLeft / ArrowRight (Browser History Navigation)
    if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      return;
    }

    // Block Backspace outside text input (Browser Back navigation)
    if (e.key === "Backspace" && !isInput) {
      e.preventDefault();
      return;
    }
  }
});

// Block Ctrl + MouseWheel Zooming in Release
window.addEventListener(
  "wheel",
  (e: WheelEvent) => {
    if (!import.meta.env.DEV && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
    }
  },
  { passive: false }
);

// Block Middle-Click Autoscroll (circle cursor) in Release
window.addEventListener("auxclick", (e: MouseEvent) => {
  if (!import.meta.env.DEV && e.button === 1) {
    e.preventDefault();
  }
});

// Block default browser right-click context menu in release mode
window.addEventListener("contextmenu", (e: MouseEvent) => {
  if (!import.meta.env.DEV) {
    const target = e.target as HTMLElement | null;
    // Allow native menu only inside text input / textarea for copy-paste
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
      return;
    }
    e.preventDefault();
  }
});

// Prevent default browser file navigation on drag-over / drop (Tauri handles it at window level)
window.addEventListener("dragover", (e) => e.preventDefault(), false);
window.addEventListener("drop", (e) => e.preventDefault(), false);

/** Detachable widgets are extra webviews with their own compact roots. */
function windowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

async function mount() {
  // Only load the root component this window actually needs.
  const label = windowLabel();
  const { default: Root } =
    label === "player-widget"
      ? await import("./components/PlayerWidget.vue")
      : label === "video-widget"
        ? await import("./components/VideoWidget.vue")
        : label === "playlist-widget"
          ? await import("./components/PlaylistWidget.vue")
          : await import("./App.vue");
  const app = createApp(Root);
  app.use(createPinia());
  app.mount("#app");
}

void mount();
