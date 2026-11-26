<script setup lang="ts">
import { computed, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Download, LockKeyhole, Plus, Trash2, Users } from "@lucide/vue";
import { useWorkspaceStore } from "../stores/workspace";
import { useToast } from "../composables/useToast";
import { api } from "../services/tauri";

const workspaceStore = useWorkspaceStore();
const toast = useToast();

const newWorkspaceName = ref("");
const recoveryInput = ref("");
const workspaceBusy = ref(false);
const copiedKey = ref(false);

async function runWorkspaceAction(action: () => Promise<void>, message: string) {
  if (workspaceBusy.value) return;
  workspaceBusy.value = true;
  try {
    await action();
    toast.success(message);
  } catch (error) {
    toast.error(String(error), "Workspace");
  } finally {
    workspaceBusy.value = false;
  }
}

async function createWorkspace() {
  const name = newWorkspaceName.value.trim();
  if (!name || workspaceBusy.value) return;
  await runWorkspaceAction(async () => {
    await workspaceStore.create(name);
    newWorkspaceName.value = "";
  }, "Workspace created. Save its recovery key now.");
}

async function renameWorkspace(item: { id: string; name: string }) {
  const name = window.prompt("Workspace name", item.name)?.trim();
  if (!name || name === item.name) return;
  await runWorkspaceAction(() => workspaceStore.rename(item.id, name), "Workspace renamed.");
}

async function deleteWorkspace(item: { id: string; name: string }) {
  if (!window.confirm(`Delete workspace "${item.name}" and everything in it? This cannot be undone.`)) return;
  await runWorkspaceAction(() => workspaceStore.remove(item.id), "Workspace deleted.");
}

async function setWorkspacePassword() {
  const password = window.prompt("New workspace password (at least 4 characters)") ?? "";
  if (!password) return;
  if (password.length < 4) {
    toast.error("Password must be at least 4 characters.", "Workspace");
    return;
  }
  await runWorkspaceAction(() => workspaceStore.setPassword(password), "Workspace password enabled.");
}

async function clearWorkspacePassword() {
  const password = window.prompt("Current workspace password") ?? "";
  if (!password) return;
  await runWorkspaceAction(() => workspaceStore.clearPassword(password), "Workspace password removed.");
}

const workspaceAvatarUrl = computed(() =>
  workspaceStore.active?.avatarPath ? convertFileSrc(workspaceStore.active.avatarPath) : null,
);

async function chooseAvatar() {
  if (workspaceBusy.value) return;
  await api.beginNativeDialog();
  try {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof path === "string") {
      await runWorkspaceAction(() => workspaceStore.setAvatar(path), "Workspace avatar updated.");
    }
  } finally {
    await api.endNativeDialog();
  }
}

async function removeAvatar() {
  await runWorkspaceAction(() => workspaceStore.clearAvatar(), "Workspace avatar removed.");
}

async function copyRecoveryKey() {
  const key = workspaceStore.lastRecoveryKey;
  if (!key) return;
  await navigator.clipboard.writeText(key);
  copiedKey.value = true;
  window.setTimeout(() => (copiedKey.value = false), 1800);
}

function downloadRecoveryKey() {
  const key = workspaceStore.lastRecoveryKey;
  if (!key) return;
  const name = workspaceStore.active?.name ?? "workspace";
  const safe = name.replace(/[^a-z0-9_-]+/gi, "_").slice(0, 40) || "workspace";
  const content = [
    "Slide Up workspace recovery key",
    `Workspace: ${name}`,
    `Workspace ID: ${workspaceStore.active?.id ?? ""}`,
    `Recovery key: ${key}`,
    "",
    "Keep this file somewhere safe. It restores access if Windows account protection is unavailable.",
  ].join("\n");
  const url = URL.createObjectURL(new Blob([content], { type: "text/plain;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `slideup-recovery-${safe}.txt`;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <div class="workspace-view">
    <div class="workspace-header">
      <span class="workspace-icon"><Users :size="20" /></span>
      <div>
        <h1>Workspaces</h1>
        <p>Manage the isolated spaces that keep each person's media separate.</p>
      </div>
    </div>

    <div class="su-panel settings-section">
      <div class="panel-body">
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">Isolated workspaces</div>
            <div class="setting-hint">Each workspace owns a separate SQLCipher-encrypted library, playlists and settings. Switching locks the current workspace first.</div>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">What stays separate</div>
            <div class="setting-hint">Library, playlists, history, playback/subtitle/video settings and covers belong to the active workspace. Theme, startup, window position and file associations are global.</div>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">Workspace profile</div>
            <div class="setting-hint">Shown in the workspace switcher. The image is stored inside this workspace only.</div>
          </div>
          <div class="setting-control ws-actions">
            <span class="profile-avatar">
              <img v-if="workspaceAvatarUrl" :src="workspaceAvatarUrl" alt="Workspace avatar" />
              <span v-else>{{ (workspaceStore.active?.name ?? "W").slice(0, 2).toUpperCase() }}</span>
            </span>
            <button type="button" :disabled="workspaceBusy" @click="chooseAvatar">Change</button>
            <button v-if="workspaceAvatarUrl" type="button" :disabled="workspaceBusy" @click="removeAvatar">Remove</button>
          </div>
        </div>
        <div v-for="item in workspaceStore.workspaces" :key="item.id" class="setting-row">
          <div class="setting-info">
            <div class="setting-name">
              {{ item.name }}
              <span v-if="item.id === workspaceStore.active?.id" class="ws-badge">Active</span>
              <span v-if="item.locked" class="ws-badge locked">Locked</span>
            </div>
            <div class="setting-hint">{{ item.id }} · {{ item.dbEncrypted ? "SQLCipher encrypted" : "Not encrypted" }}</div>
          </div>
          <div class="setting-control ws-actions">
            <button
              v-if="item.id !== workspaceStore.active?.id"
              type="button"
              :disabled="workspaceBusy"
              @click="runWorkspaceAction(() => workspaceStore.switchTo(item.id), 'Workspace switched.')"
            >Switch</button>
            <template v-else>
              <button type="button" :disabled="workspaceBusy" @click="runWorkspaceAction(() => workspaceStore.lock(), 'Workspace locked.')">
                <LockKeyhole :size="14" /> Lock
              </button>
              <button type="button" :disabled="workspaceBusy" @click="renameWorkspace(item)">Rename</button>
              <button v-if="item.id !== 'default'" type="button" :disabled="workspaceBusy" @click="deleteWorkspace(item)">
                <Trash2 :size="14" />
              </button>
            </template>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">Workspace password</div>
            <div class="setting-hint">Require a password to unlock this workspace. Without one, Windows account protection is used.</div>
          </div>
          <div class="setting-control ws-actions">
            <button type="button" :disabled="workspaceBusy" @click="setWorkspacePassword">
              {{ workspaceStore.active?.passwordProtected ? "Change" : "Set" }} password
            </button>
            <button
              v-if="workspaceStore.active?.passwordProtected"
              type="button"
              :disabled="workspaceBusy"
              @click="clearWorkspacePassword"
            >Remove</button>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">New workspace</div>
            <div class="setting-hint">A recovery key is shown once after creation. Keep it somewhere safe.</div>
          </div>
          <div class="setting-control ws-actions">
            <input v-model="newWorkspaceName" type="text" placeholder="Workspace name" aria-label="New workspace name" @keyup.enter="createWorkspace" />
            <button type="button" :disabled="workspaceBusy || !newWorkspaceName.trim()" @click="createWorkspace">
              <Plus :size="14" /> Create
            </button>
          </div>
        </div>
        <div v-if="workspaceStore.lastRecoveryKey" class="recovery-callout">
          <div>
            <strong>Recovery key</strong>
            <code>{{ workspaceStore.lastRecoveryKey }}</code>
          </div>
          <div class="recovery-actions">
            <button type="button" @click="copyRecoveryKey">{{ copiedKey ? "Copied" : "Copy" }}</button>
            <button type="button" @click="downloadRecoveryKey"><Download :size="13" /> Download</button>
          </div>
        </div>
        <div class="setting-row">
          <div class="setting-info">
            <div class="setting-name">Restore access</div>
            <div class="setting-hint">Use the recovery key if Windows account protection can no longer unlock a workspace.</div>
          </div>
          <div class="setting-control ws-actions">
            <input v-model="recoveryInput" type="password" placeholder="Recovery key" aria-label="Workspace recovery key" />
            <button
              type="button"
              :disabled="workspaceBusy || !recoveryInput.trim()"
              @click="runWorkspaceAction(async () => { await workspaceStore.restore(recoveryInput); recoveryInput = ''; }, 'Workspace restored.')"
            >Restore</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.workspace-view { height: 100%; overflow-y: auto; padding: 20px 24px 40px; }
.workspace-header { display: flex; align-items: center; gap: 12px; margin-bottom: 18px; }
.workspace-icon { display: grid; width: 40px; height: 40px; place-items: center; border-radius: 12px; color: #a78bfa; background: rgb(139 92 246 / 14%); }
.workspace-header h1 { margin: 0; font-size: 19px; font-weight: 700; color: var(--text-primary); }
.workspace-header p { margin: 3px 0 0; font-size: 12px; color: var(--text-muted); }
.panel-body { padding: 0.2rem 1.25rem 0.5rem; }
.setting-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; column-gap: 1.5rem; padding: 0.8rem 0; border-bottom: 1px solid var(--border-subtle); }
.setting-row:last-child { border-bottom: 0; }
.setting-info { min-width: 0; }
.setting-name { font-size: 0.82rem; font-weight: 600; color: var(--text-primary); }
.setting-hint { margin-top: 0.15rem; font-size: 0.7rem; line-height: 1.45; color: var(--text-muted); }
.setting-control { display: flex; align-items: center; justify-content: flex-end; gap: 0.5rem; justify-self: end; }

.ws-actions { flex-wrap: wrap; }
.ws-actions button { display: inline-flex; height: 2rem; align-items: center; gap: 0.3rem; padding: 0 0.7rem; border: 1px solid var(--border-subtle); border-radius: 0.5rem; background: var(--bg-elevated); color: var(--text-secondary); cursor: pointer; font-size: 0.72rem; }
.ws-actions button:hover:not(:disabled) { color: var(--text-primary); border-color: var(--border-strong); }
.ws-actions button:disabled { opacity: 0.45; cursor: not-allowed; }
.ws-actions input { width: 11rem; height: 2rem; padding: 0 0.6rem; border: 1px solid var(--border-subtle); border-radius: 0.5rem; background: var(--bg-elevated); color: var(--text-primary); font-size: 0.72rem; }
.profile-avatar { display: grid; width: 2rem; height: 2rem; overflow: hidden; place-items: center; border-radius: 50%; background: linear-gradient(135deg, #5b4bb5, #2e6cb0); color: #fff; font-size: 0.62rem; font-weight: 700; }
.profile-avatar img { width: 100%; height: 100%; object-fit: cover; }
.ws-badge { margin-left: 0.4rem; padding: 0.1rem 0.4rem; border-radius: 0.35rem; background: rgb(139 92 246 / 18%); color: #a78bfa; font-size: 0.62rem; font-weight: 600; }
.ws-badge.locked { background: rgb(248 113 113 / 16%); color: #f87171; }
.recovery-callout { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin: 0.6rem 0; padding: 0.75rem 0.9rem; border: 1px solid rgb(139 92 246 / 35%); border-radius: 0.65rem; background: rgb(139 92 246 / 8%); }
.recovery-callout strong { display: block; margin-bottom: 0.3rem; font-size: 0.7rem; color: var(--text-secondary); }
.recovery-callout code { word-break: break-all; font-size: 0.7rem; color: var(--text-primary); }
.recovery-actions { display: flex; flex-shrink: 0; gap: 0.4rem; }
.recovery-callout button { display: inline-flex; flex-shrink: 0; height: 2rem; align-items: center; gap: 0.3rem; padding: 0 0.7rem; border: 1px solid var(--border-subtle); border-radius: 0.5rem; background: var(--bg-elevated); color: var(--text-secondary); cursor: pointer; font-size: 0.72rem; }
</style>
