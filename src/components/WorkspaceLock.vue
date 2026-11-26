<script setup lang="ts">
import { computed, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ArrowLeft, KeyRound, LockKeyhole, LogOut } from "@lucide/vue";
import { useWorkspaceStore } from "../stores/workspace";

const workspace = useWorkspaceStore();
const error = ref("");
const busy = ref(false);
const password = ref("");
const recoveryKey = ref("");
const showRecovery = ref(false);

const avatarUrl = computed(() => {
  const path = workspace.selected?.avatarPath ?? workspace.active?.avatarPath;
  return path ? convertFileSrc(path) : null;
});

async function unlock() {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    if (workspace.selected?.passwordProtected) await workspace.unlockWithPassword(password.value);
    else await workspace.unlock();
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
    for (const window of await WebviewWindow.getAll()) {
      await window.close().catch(() => {});
    }
  } finally {
    busy.value = false;
  }
}

async function backToMySpace() {
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

async function restore() {
  if (busy.value || !recoveryKey.value.trim()) return;
  busy.value = true;
  error.value = "";
  try {
    await workspace.restore(recoveryKey.value);
    recoveryKey.value = "";
    showRecovery.value = false;
    if (!workspace.selected?.passwordProtected) await workspace.unlock();
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="workspace-lock">
    <div class="lock-card">
      <button class="lock-exit" type="button" :disabled="busy" @click="exitApp">
        <LogOut :size="14" /> Exit
      </button>
      <div class="lock-icon"><LockKeyhole :size="30" /></div>
      <div v-if="avatarUrl" class="lock-avatar"><img :src="avatarUrl" alt="Workspace avatar" /></div>
      <h1>Workspace is locked</h1>
      <p class="lock-subtitle">
        Media, playlists and settings for
        <strong>{{ workspace.selected?.name ?? workspace.active?.name ?? "this workspace" }}</strong>
        are stored in an encrypted database. Nothing is accessible until it is unlocked.
      </p>

      <input
        v-if="workspace.selected?.passwordProtected"
        v-model="password"
        class="password-input"
        type="password"
        placeholder="Workspace password"
        aria-label="Workspace password"
        autocomplete="current-password"
        @keyup.enter="unlock"
      />
      <button
        class="unlock-button"
        type="button"
        :disabled="busy || (workspace.selected?.passwordProtected && !password)"
        @click="unlock"
      >
        {{ busy ? "Unlocking…" : "Unlock workspace" }}
      </button>
      <p class="lock-hint">
        {{
          workspace.selected?.passwordProtected
            ? "Enter the password set for this workspace. No app data is read before this step."
            : "Unlocking uses Windows account protection. No app data is read before this step."
        }}
      </p>

      <button
        v-if="workspace.canGoBack"
        class="lock-back"
        type="button"
        :disabled="busy"
        @click="backToMySpace"
      >
        <ArrowLeft :size="14" /> Back to my space
      </button>

      <button class="recovery-toggle" type="button" @click="showRecovery = !showRecovery">
        <KeyRound :size="14" />
        {{ showRecovery ? "Hide recovery key" : "Lost access? Use recovery key" }}
      </button>
      <div v-if="showRecovery" class="recovery-form">
        <input
          v-model="recoveryKey"
          type="password"
          placeholder="Paste the recovery key shown at creation"
          aria-label="Workspace recovery key"
          @keyup.enter="restore"
        />
        <button type="button" :disabled="busy || !recoveryKey.trim()" @click="restore">Restore</button>
      </div>
      <p v-if="error" class="lock-error" role="alert">{{ error }}</p>
    </div>
  </div>
</template>

<style scoped>
.workspace-lock {
  position: fixed;
  z-index: 9999;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: radial-gradient(circle at 50% 30%, #1b2444 0%, #080d1d 65%);
  color: var(--text-primary);
}

.lock-card {
  position: relative;
  width: min(440px, 100%);
  padding: 32px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 18px;
  background: var(--bg-panel, #121a30);
  box-shadow: 0 24px 70px rgb(0 0 0 / 45%);
  text-align: center;
}

.lock-exit {
  position: absolute;
  top: 12px;
  right: 12px;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 6px 9px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 8px;
  color: var(--text-secondary, #97a3c4);
  background: var(--bg-elevated, #1b2444);
  cursor: pointer;
  font-size: 11px;
}

.lock-exit:hover:not(:disabled) { color: var(--text-primary); }
.lock-exit:disabled { opacity: 0.5; cursor: not-allowed; }

.lock-icon {
  display: grid;
  width: 56px;
  height: 56px;
  margin: 0 auto 18px;
  place-items: center;
  border-radius: 16px;
  color: #a78bfa;
  background: rgb(139 92 246 / 14%);
}

.lock-avatar { width: 52px; height: 52px; margin: -8px auto 14px; overflow: hidden; border: 2px solid rgb(139 92 246 / 45%); border-radius: 50%; }
.lock-avatar img { width: 100%; height: 100%; object-fit: cover; }

.lock-card h1 {
  margin: 0 0 10px;
  font-size: 22px;
}

.lock-subtitle {
  margin: 0 0 24px;
  color: var(--text-secondary, #97a3c4);
  font-size: 13px;
  line-height: 1.55;
}

.password-input {
  width: 100%;
  margin-bottom: 12px;
  padding: 11px 12px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 9px;
  color: var(--text-primary);
  background: var(--bg-base, #0b1120);
  font-size: 13px;
}

.unlock-button {
  width: 100%;
  padding: 12px 16px;
  border: 0;
  border-radius: 10px;
  color: #fff;
  background: var(--accent-purple, #8b5cf6);
  cursor: pointer;
  font-weight: 600;
}

.unlock-button:disabled {
  opacity: 0.6;
  cursor: wait;
}

.lock-hint {
  margin: 12px 0 18px;
  color: var(--text-muted, #6b7697);
  font-size: 11px;
}

.lock-back {
  display: flex;
  gap: 6px;
  align-items: center;
  justify-content: center;
  width: 100%;
  margin-top: 12px;
  padding: 9px 12px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 9px;
  color: var(--text-secondary, #97a3c4);
  background: none;
  cursor: pointer;
  font-size: 12px;
}

.lock-back:hover:not(:disabled) { color: var(--text-primary); }
.lock-back:disabled { opacity: 0.5; cursor: not-allowed; }

.recovery-toggle {
  display: inline-flex;
  gap: 6px;
  align-items: center;
  border: 0;
  color: var(--text-secondary, #97a3c4);
  background: none;
  cursor: pointer;
  font-size: 12px;
}

.recovery-form {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.recovery-form input {
  flex: 1;
  min-width: 0;
  padding: 9px 10px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 8px;
  color: var(--text-primary);
  background: var(--bg-base, #0b1120);
  font-size: 12px;
}

.recovery-form button {
  padding: 9px 12px;
  border: 1px solid var(--border-color, #2a3350);
  border-radius: 8px;
  color: var(--text-primary);
  background: var(--bg-panel, #121a30);
  cursor: pointer;
  font-size: 12px;
}

.lock-error {
  margin: 14px 0 0;
  color: #f87171;
  font-size: 12px;
}
</style>
