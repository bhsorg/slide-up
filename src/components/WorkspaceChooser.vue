<script setup lang="ts">
import { ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ArrowLeft, LogOut, Users } from "@lucide/vue";
import { useWorkspaceStore } from "../stores/workspace";

const workspace = useWorkspaceStore();
const busy = ref(false);
const error = ref("");

function avatarUrl(path: string | null): string | null {
  return path ? convertFileSrc(path) : null;
}

async function choose(id: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.chooseWorkspace(id);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    busy.value = false;
  }
}

async function back() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.backToMySpace();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    busy.value = false;
  }
}

async function exitApp() {
  if (busy.value) return;
  busy.value = true;
  try {
    // Closing windows lets WebView2 tear down cleanly; process::exit(0)
    // produced Chrome_WidgetWin_0 unregister errors (1412) on Windows.
    for (const window of await WebviewWindow.getAll()) {
      await window.close().catch(() => {});
    }
  } finally {
    busy.value = false;
  }
}
</script>


<template>
  <div class="workspace-chooser">
    <div class="chooser-card">
      <button class="chooser-exit" type="button" :disabled="busy" @click="exitApp">
        <LogOut :size="14" /> Exit
      </button>
      <div class="chooser-icon"><Users :size="30" /></div>
      <h1>Choose a workspace</h1>
      <p class="chooser-subtitle">Each workspace keeps its own media, playlists and settings, isolated in an encrypted database.</p>
      <div class="workspace-list">
        <button
          v-for="item in workspace.workspaces"
          :key="item.id"
          type="button"
          class="workspace-item"
          :disabled="busy"
          :class="{ current: item.id === workspace.active?.id }"
          @click="choose(item.id)"
        >
          <span class="workspace-avatar">
            <img v-if="avatarUrl(item.avatarPath)" :src="avatarUrl(item.avatarPath) ?? undefined" :alt="item.name" />
            <template v-else>{{ item.name.slice(0, 1).toUpperCase() }}</template>
          </span>
          <span class="workspace-detail">
            <strong>{{ item.name }}</strong>
            <small>
              {{ item.passwordProtected ? "Password protected" : "Windows account protection" }}
              <span v-if="item.id === workspace.active?.id"> · Last used</span>
            </small>
          </span>
        </button>
      </div>
      <p v-if="error" class="chooser-error" role="alert">{{ error }}</p>
      <button
        v-if="workspace.canGoBack"
        class="chooser-back"
        type="button"
        :disabled="busy"
        @click="back"
      >
        <ArrowLeft :size="14" /> Back to my space
      </button>
      <p class="chooser-hint">A workspace protected by a password will ask for it next.</p>
    </div>
  </div>
</template>

<style scoped>
.workspace-chooser {
  position: fixed;
  z-index: 9998;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: radial-gradient(circle at 50% 30%, #1b2444 0%, #080d1d 65%);
  color: var(--text-primary);
}

.chooser-card {
  position: relative;
  width: min(460px, 100%);
  padding: 32px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 18px;
  background: var(--bg-panel, #121a30);
  box-shadow: 0 24px 70px rgb(0 0 0 / 45%);
  text-align: center;
}

.chooser-icon {
  display: grid;
  width: 56px;
  height: 56px;
  margin: 0 auto 18px;
  place-items: center;
  border-radius: 16px;
  color: #a78bfa;
  background: rgb(139 92 246 / 14%);
}

.chooser-card h1 { margin: 0 0 10px; font-size: 22px; }
.chooser-subtitle { margin: 0 0 22px; color: var(--text-secondary, #97a3c4); font-size: 13px; line-height: 1.55; }
.workspace-list { display: grid; gap: 10px; }

.workspace-item {
  display: flex;
  gap: 12px;
  align-items: center;
  width: 100%;
  padding: 12px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 12px;
  color: var(--text-primary);
  background: var(--bg-elevated, #18213b);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s ease, transform 0.15s ease;
}

.workspace-item:hover:not(:disabled) { border-color: #8b5cf6; transform: translateY(-1px); }
.workspace-item.current { border-color: rgb(139 92 246 / 55%); }
.workspace-item:disabled { opacity: 0.6; cursor: wait; }

.workspace-avatar {
  display: grid;
  flex-shrink: 0;
  width: 38px;
  height: 38px;
  place-items: center;
  border-radius: 11px;
  color: #fff;
  background: linear-gradient(135deg, #8b5cf6, #3b82f6);
  font-weight: 700;
}

.workspace-detail { display: grid; gap: 3px; min-width: 0; }
.workspace-detail strong { font-size: 13px; }
.workspace-detail small { color: var(--text-muted, #6b7697); font-size: 11px; }
.workspace-avatar img { width: 100%; height: 100%; object-fit: cover; }
.chooser-error { margin: 16px 0 0; color: #f87171; font-size: 12px; }
.chooser-back { display: inline-flex; align-items: center; gap: 0.35rem; margin-top: 12px; padding: 6px 8px; border: 0; border-radius: 8px; color: var(--text-secondary, #97a3c4); background: none; cursor: pointer; font-size: 12px; }
.chooser-back:hover:not(:disabled) { color: var(--text-primary); }
.chooser-back:disabled { opacity: 0.5; cursor: not-allowed; }
.chooser-hint { margin: 18px 0 0; color: var(--text-muted, #6b7697); font-size: 11px; }
.chooser-exit { position: absolute; top: 12px; right: 12px; display: inline-flex; align-items: center; gap: 0.3rem; padding: 6px 9px; border: 1px solid var(--border-color, #2a3350); border-radius: 8px; color: var(--text-secondary, #97a3c4); background: var(--bg-elevated, #1b2444); cursor: pointer; font-size: 11px; }
.chooser-exit:hover:not(:disabled) { color: var(--text-primary); }
.chooser-exit:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
