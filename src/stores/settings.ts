import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { api, events, onEvent } from "../services/tauri";
import type { AppSettings, Shortcuts, Theme } from "../types/settings";
import { defaultSettings, defaultShortcuts } from "../types/settings";

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<AppSettings>(defaultSettings());
  const loaded = ref(false);
  const saving = ref(false);

  const theme = computed<Theme>(() => settings.value.theme);
  const shortcuts = computed<Shortcuts>(() => settings.value.shortcuts);

  function applyTheme(value: Theme) {
    document.documentElement.dataset.theme = value;
  }

  async function load() {
    try {
      settings.value = await api.getSettings();
    } catch {
      settings.value = defaultSettings();
    }
    applyTheme(settings.value.theme);
    loaded.value = true;
  }

  async function save() {
    saving.value = true;
    try {
      await api.updateSettings(settings.value);
    } finally {
      saving.value = false;
    }
  }

  async function patch(partial: Partial<AppSettings>) {
    settings.value = { ...settings.value, ...partial };
    applyTheme(settings.value.theme);
    await save();
  }

  async function setTheme(value: Theme) {
    await patch({ theme: value });
  }

  async function setShortcut(action: keyof Shortcuts, combo: string) {
    const next = { ...settings.value.shortcuts, [action]: combo };
    await patch({ shortcuts: next });
  }

  async function resetShortcuts() {
    await patch({ shortcuts: defaultShortcuts() });
  }

  async function resetAll() {
    settings.value = await api.resetSettings();
    applyTheme(settings.value.theme);
  }

  let dispose: (() => void) | null = null;
  async function initialize() {
    void load();
    if (dispose) return;
    dispose = await onEvent<AppSettings>(events.settingsChanged, (payload) => {
      settings.value = payload;
      applyTheme(payload.theme);
    });
  }

  function disposeListeners() {
    dispose?.();
    dispose = null;
  }

  return {
    settings,
    loaded,
    saving,
    theme,
    shortcuts,
    load,
    save,
    patch,
    setTheme,
    setShortcut,
    resetShortcuts,
    resetAll,
    initialize,
    disposeListeners,
  };
});
