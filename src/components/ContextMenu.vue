<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { ContextMenuItem } from "../types/contextMenu";

const props = defineProps<{
  x: number;
  y: number;
  items: ContextMenuItem[];
}>();

const emit = defineEmits<{ close: []; select: [index: number] }>();

const menu = ref<HTMLElement | null>(null);

const style = computed(() => {
  const width = 210;
  const height = Math.min(props.items.length * 32 + 12, 380);
  const left = Math.max(8, Math.min(props.x, window.innerWidth - width - 8));
  const top = Math.max(8, Math.min(props.y, window.innerHeight - height - 8));
  return { left: `${left}px`, top: `${top}px` };
});

function onPointerDown(event: PointerEvent) {
  if (menu.value && !menu.value.contains(event.target as Node)) emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

function onResize() {
  emit("close");
}

onMounted(() => {
  window.addEventListener("pointerdown", onPointerDown, true);
  window.addEventListener("keydown", onKeydown, true);
  window.addEventListener("resize", onResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", onPointerDown, true);
  window.removeEventListener("keydown", onKeydown, true);
  window.removeEventListener("resize", onResize);
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="menu"
      class="fixed z-[9999] min-w-[190px] overflow-hidden rounded-xl border border-line-strong bg-elevated/95 p-1.5 shadow-2xl backdrop-blur-xl"
      :style="style"
      @contextmenu.prevent
      @click.stop
    >
      <template v-for="(item, index) in items" :key="`${item.label}-${index}`">
        <div v-if="item.separator" class="my-1 h-px bg-line" role="separator" />
        <button
          v-else
          type="button"
          class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[12px] text-ink transition-colors hover:bg-active disabled:cursor-not-allowed disabled:opacity-40"
          :class="item.danger ? 'hover:!text-[#ff5b8d]' : ''"
          :disabled="item.disabled"
          @click="emit('select', index); emit('close')"
        >
          <component :is="item.icon" v-if="item.icon" :size="14" class="shrink-0 text-faint" />
          <span class="truncate">{{ item.label }}</span>
        </button>
      </template>
    </div>
  </Teleport>
</template>
