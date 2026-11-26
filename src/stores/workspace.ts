import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  api,
  events,
  onEvent,
  type WorkspaceCreation,
  type WorkspaceInfo,
} from "../services/tauri";

export const useWorkspaceStore = defineStore("workspace", () => {
  const workspaces = ref<WorkspaceInfo[]>([]);
  const active = ref<WorkspaceInfo | null>(null);
  const ready = ref(false);
  const lastRecoveryKey = ref<string | null>(null);

  // Startup gate: after splash the user picks a workspace (when there is more
  // than one) and supplies a password when that workspace requires one.
  const pending = ref<WorkspaceInfo | null>(null);
  const selectedId = ref<string | null>(null);
  const previousId = ref<string | null>(null);
  const chooserVisible = ref(false);
  const bootReady = ref(false);

  const locked = computed(() => !active.value || active.value.locked);
  const selected = computed(
    () =>
      workspaces.value.find((item) => item.id === selectedId.value) ??
      (selectedId.value === pending.value?.id ? pending.value : null),
  );

  // Only offer "Back to my space" when there is a real origin to return to.
  const canGoBack = computed(() => {
    const origin = previousId.value ?? pending.value?.id ?? active.value?.id;
    return (
      !!origin &&
      origin !== selectedId.value &&
      workspaces.value.some((item) => item.id === origin)
    );
  });

  async function refresh() {
    workspaces.value = await api.listWorkspaces();
    active.value = await api.activeWorkspace();
    ready.value = true;
  }

  async function create(name: string): Promise<WorkspaceCreation> {
    const created = await api.createWorkspace(name);
    lastRecoveryKey.value = created.recoveryKey;
    await refresh();
    return created;
  }

  async function switchTo(id: string) {
    // Go through the same gate as the startup chooser so a protected
    // workspace always asks for its password before any data is loaded.
    selectedId.value = id;
    await activateSelected();
  }

  async function lock() {
    await api.lockWorkspace();
    await refresh();
  }

  async function unlock() {
    await api.unlockWorkspace(selectedId.value ?? undefined);
    await refresh();
    if (!locked.value) bootReady.value = true;
  }

  async function unlockWithPassword(password: string) {
    const id = selectedId.value ?? active.value?.id ?? pending.value?.id;
    if (!id) throw new Error("No workspace is selected.");
    await api.unlockWorkspaceWithPassword(id, password);
    await refresh();
    if (!locked.value) bootReady.value = true;
  }

  async function setPassword(password: string) {
    await api.setWorkspacePassword(password);
    await refresh();
  }

  async function clearPassword(password: string) {
    await api.clearWorkspacePassword(password);
    await refresh();
  }

  async function rename(id: string, name: string) {
    await api.renameWorkspace(id, name);
    await refresh();
  }

  async function setAvatar(path: string) {
    await api.setWorkspaceAvatar(path);
    await refresh();
  }

  async function clearAvatar() {
    await api.clearWorkspaceAvatar();
    await refresh();
  }

  async function remove(id: string) {
    await api.deleteWorkspace(id);
    await refresh();
  }

  async function restore(recoveryKey: string) {
    await api.restoreWorkspace(recoveryKey);
    await refresh();
  }

  /** Activate the selected workspace, prompting for a password only if needed. */
  async function activateSelected() {
    const id = selectedId.value;
    if (!id) return;
    // Remember where we came from so the user can return after switching.
    // Remember where we came from so a wrong pick can be undone.
    // At startup the active workspace may be null while `pending` still holds
    // the last workspace the user was in.
    const origin = active.value?.id ?? pending.value?.id;
    if (origin && origin !== id) {
      previousId.value = origin;
    }
    const target = selected.value;
    if (target?.passwordProtected) {
      // Never auto-unlock a password-protected workspace. Select it while
      // locked so the lock screen asks for its password before any data loads.
      await api.selectWorkspace(id);
      await refresh();
      chooserVisible.value = false;
      return;
    }
    if (active.value?.id === id) {
      if (target?.locked) await api.unlockWorkspace(id);
    } else {
      await api.switchWorkspace(id);
    }
    await refresh();
    if (!locked.value) {
      bootReady.value = true;
      chooserVisible.value = false;
    }
  }

  async function chooseWorkspace(id: string) {
    selectedId.value = id;
    await activateSelected();
  }

  /** Return to the workspace the user came from after switching away. */
  async function backToMySpace() {
    const target = previousId.value && workspaces.value.some((item) => item.id === previousId.value)
      ? previousId.value
      : (pending.value?.id ?? "default");
    if (active.value?.id === target) return;
    await switchTo(target);
  }

  let dispose: (() => void) | null = null;
  async function initialize() {
    await refresh();
    pending.value = await api.pendingWorkspace().catch(() => null);
    if (!dispose) {
      dispose = await onEvent<WorkspaceInfo>(events.workspaceChanged, (payload) => {
        active.value = payload;
        const index = workspaces.value.findIndex((item) => item.id === payload.id);
        if (index >= 0) workspaces.value[index] = payload;
        else workspaces.value.push(payload);
      });
    }

    if (workspaces.value.length > 1) {
      previousId.value = pending.value?.id ?? active.value?.id ?? null;
      selectedId.value = pending.value?.id ?? active.value?.id ?? workspaces.value[0].id;
      chooserVisible.value = true;
      return;
    }

    selectedId.value = workspaces.value[0]?.id ?? pending.value?.id ?? null;
    await activateSelected();
  }

  function disposeListeners() {
    dispose?.();
    dispose = null;
  }

  return {
    workspaces,
    active,
    ready,
    locked,
    lastRecoveryKey,
    pending,
    selectedId,
    previousId,
    selected,
    canGoBack,
    chooserVisible,
    bootReady,
    refresh,
    create,
    switchTo,
    lock,
    unlock,
    unlockWithPassword,
    setPassword,
    clearPassword,
    rename,
    setAvatar,
    clearAvatar,
    remove,
    restore,
    activateSelected,
    chooseWorkspace,
    backToMySpace,
    initialize,
    disposeListeners,
  };
});
