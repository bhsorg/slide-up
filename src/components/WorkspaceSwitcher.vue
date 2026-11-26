<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, ChevronDown, ImagePlus, LockKeyhole, Moon, Settings2, Sun } from "@lucide/vue";
import { api } from "../services/tauri";
import { useSettingsStore } from "../stores/settings";
import { useWorkspaceStore } from "../stores/workspace";

const emit = defineEmits<{ settings: [] }>();
const workspace = useWorkspaceStore();
const settings = useSettingsStore();
const open = ref(false);
const root = ref<HTMLElement | null>(null);
const busy = ref(false);

const activeName = computed(() => workspace.active?.name ?? "Workspace");
const avatarUrl = computed(() =>
  workspace.active?.avatarPath ? convertFileSrc(workspace.active.avatarPath) : null,
);
const initials = computed(() => activeName.value.trim().slice(0, 2).toUpperCase() || "W");

function onWindowClick(event: MouseEvent) {
  if (root.value && !root.value.contains(event.target as Node)) open.value = false;
}

onMounted(() => window.addEventListener("click", onWindowClick));
onBeforeUnmount(() => window.removeEventListener("click", onWindowClick));

function avatarFor(path: string | null | undefined): string | undefined {
  return path ? convertFileSrc(path) : undefined;
}

function nameInitials(name: string): string {
  return name.trim().slice(0, 2).toUpperCase() || "W";
}

async function select(id: string) {
  if (busy.value || id === workspace.active?.id) {
    open.value = false;
    return;
  }
  busy.value = true;
  try {
    await workspace.switchTo(id);
    open.value = false;
  } finally {
    busy.value = false;
  }
}

async function backToMySpace() {
  if (busy.value) return;
  busy.value = true;
  try {
    await workspace.backToMySpace();
    open.value = false;
  } finally {
    busy.value = false;
  }
}

async function pickAvatar() {
  if (busy.value) return;
  await api.beginNativeDialog();
  try {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (typeof path === "string") await workspace.setAvatar(path);
  } finally {
    await api.endNativeDialog();
  }
}
</script>

<template>
  <div ref="root" class="workspace-switcher relative">
    <button
      type="button"
      class="workspace-trigger"
      :title="`Workspace: ${activeName}`"
      @click.stop="open = !open"
    >
      <span v-if="avatarUrl" class="ws-avatar"><img :src="avatarUrl" :alt="activeName" /></span>
      <span v-else class="ws-avatar placeholder">{{ initials }}</span>
      <span class="ws-name">{{ activeName }}</span>
      <ChevronDown :size="12" class="text-faint" />
    </button>

    <Transition name="fade-scale">
      <div v-if="open" class="workspace-menu" @click.stop>
        <div class="menu-heading">Your workspaces</div>
        <button
          v-for="item in workspace.workspaces"
          :key="item.id"
          type="button"
          class="ws-menu-item"
          :class="{ current: item.id === workspace.active?.id }"
          :disabled="busy"
          @click="select(item.id)"
        >
          <span v-if="item.avatarPath" class="ws-avatar small">
            <img :src="avatarFor(item.avatarPath)" :alt="item.name" />
          </span>
          <span v-else class="ws-avatar small placeholder">{{ nameInitials(item.name) }}</span>
          <span class="ws-menu-name">
            {{ item.name }}
            <small>{{ item.locked ? "Locked" : item.passwordProtected ? "Password" : "Windows account" }}</small>
          </span>
          <LockKeyhole v-if="item.locked" :size="13" class="text-faint" />
        </button>

        <div class="my-1 h-px bg-line" />
        <button type="button" class="ws-menu-action" title="Light theme" @click="settings.setTheme('light')">
          <Sun :size="14" /> Light theme
        </button>
        <button type="button" class="ws-menu-action" title="Dark theme" @click="settings.setTheme('dark')">
          <Moon :size="14" /> Dark theme
        </button>
        <div class="my-1 h-px bg-line" />
        <button
          v-if="workspace.canGoBack"
          type="button"
          class="ws-menu-action"
          :disabled="busy"
          @click="backToMySpace"
        >
          <ArrowLeft :size="14" /> Back to my space
        </button>
        <button type="button" class="ws-menu-action" @click="pickAvatar">
          <ImagePlus :size="14" /> Change avatar
        </button>
        <button
          type="button"
          class="ws-menu-action"
          @click="
            open = false;
            emit('settings');
          "
        >
          <Settings2 :size="14" /> Manage workspaces
        </button>
        <button
          v-if="workspace.active && !workspace.active.locked"
          type="button"
          class="ws-menu-action"
          :disabled="busy"
          @click="
            open = false;
            void workspace.lock();
          "
        >
          <LockKeyhole :size="14" /> Lock this workspace
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.workspace-trigger {
  display: flex;
  height: 1.9rem;
  align-items: center;
  gap: 0.4rem;
  padding: 0 0.5rem 0 0.25rem;
  border: 1px solid var(--border-line, #2a3350);
  border-radius: 999px;
  color: var(--text-secondary, #97a3c4);
  background: var(--bg-panel, #121a30);
  cursor: pointer;
  font-size: 0.72rem;
}
.workspace-trigger:hover { color: var(--text-primary); }
.ws-avatar {
  display: grid;
  width: 1.35rem;
  height: 1.35rem;
  flex-shrink: 0;
  overflow: hidden;
  place-items: center;
  border-radius: 50%;
  background: linear-gradient(135deg, #8b5cf6, #4f8cff);
  color: #fff;
  font-size: 0.52rem;
  font-weight: 700;
}
.ws-avatar img { width: 100%; height: 100%; object-fit: cover; }
.ws-avatar.placeholder { background: linear-gradient(135deg, #5b4bb5, #2e6cb0); }
.ws-avatar.small { width: 1.6rem; height: 1.6rem; font-size: 0.6rem; }
.ws-name { max-width: 7rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.workspace-menu {
  position: absolute;
  right: 0;
  top: calc(100% + 0.4rem);
  z-index: 60;
  width: 15rem;
  padding: 0.4rem;
  border: 1px solid var(--border-line-strong, #3a4568);
  border-radius: 0.75rem;
  background: var(--bg-elevated, #18213a);
  box-shadow: 0 18px 40px rgb(0 0 0 / 40%);
}
.menu-heading { padding: 0.35rem 0.5rem; color: var(--text-muted, #6b7697); font-size: 0.62rem; text-transform: uppercase; letter-spacing: 0.08em; }
.ws-menu-item, .ws-menu-action {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 0.55rem;
  padding: 0.45rem 0.5rem;
  border: 0;
  border-radius: 0.5rem;
  color: var(--text-secondary, #97a3c4);
  background: none;
  cursor: pointer;
  text-align: left;
  font-size: 0.72rem;
}
.ws-menu-item:hover:not(:disabled), .ws-menu-action:hover:not(:disabled) { background: var(--bg-hover, #232d4a); color: var(--text-primary); }
.ws-menu-item.current { background: rgb(139 92 246 / 12%); color: var(--text-primary); }
.ws-menu-item:disabled, .ws-menu-action:disabled { opacity: 0.5; cursor: not-allowed; }
.ws-menu-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ws-menu-name small { display: block; color: var(--text-muted, #6b7697); font-size: 0.6rem; }
</style>
