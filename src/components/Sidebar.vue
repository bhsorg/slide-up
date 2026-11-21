<script setup lang="ts">
import { House, Music2, Clapperboard, ListMusic, Heart, History, Settings, ChevronRight } from "@lucide/vue";
import type { ViewId } from "../types/settings";

defineProps<{ active: ViewId }>();
const emit = defineEmits<{ navigate: [view: ViewId] }>();

const navItems: Array<{ id: ViewId; label: string; icon: typeof House }> = [
  { id: "home", label: "Home", icon: House },
  { id: "music", label: "Music Library", icon: Music2 },
  { id: "videos", label: "Video Library", icon: Clapperboard },
  { id: "playlists", label: "Playlists", icon: ListMusic },
  { id: "favorites", label: "Favorites", icon: Heart },
  { id: "recent", label: "Recently Played", icon: History },
  { id: "settings", label: "Settings", icon: Settings },
];

const quickAccess: Array<{ id: ViewId; label: string; hint: string; icon: typeof House; color: string }> = [
  { id: "music", label: "Music", hint: "Your Music", icon: Music2, color: "from-purple to-blue" },
  { id: "videos", label: "Videos", hint: "Your Videos", icon: Clapperboard, color: "from-blue to-purple" },
  { id: "favorites", label: "Favorites", hint: "Liked Media", icon: Heart, color: "from-[#ff4d8d] to-[#ff9a5c]" },
];
</script>

<template>
  <aside class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col overflow-hidden border-r border-line bg-secondary">
    <!-- Main Navigation Items starting directly at the top -->
    <nav class="flex flex-col gap-1 px-2.5 pt-2">
      <button
        v-for="item in navItems"
        :key="item.id"
        type="button"
        class="sidebar-nav-item"
        :class="{ active: active === item.id }"
        @click="emit('navigate', item.id)"
      >
        <component :is="item.icon" :size="16" :stroke-width="1.8" />
        <span class="flex-1">{{ item.label }}</span>
      </button>
    </nav>

    <div class="mx-3 my-2.5 h-px bg-line" />

    <!-- Quick Access Section -->
    <div class="px-2.5">
      <div class="mb-1.5 px-1 text-[10px] font-semibold uppercase tracking-wider text-faint">Quick Access</div>
      <div class="flex flex-col gap-1.5">
        <button
          v-for="item in quickAccess"
          :key="item.label"
          type="button"
          class="quick-access-btn"
          @click="emit('navigate', item.id)"
        >
          <span
            class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-white"
            :class="item.id === 'music' ? 'bg-[#7a2bff]' : item.id === 'videos' ? 'bg-[#f59e0b]' : 'bg-[#ef4444]'"
          >
            <component :is="item.icon" :size="14" :stroke-width="2" />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[11.5px] font-semibold text-ink">{{ item.label }}</span>
            <span class="block truncate text-[9.5px] text-faint">{{ item.hint }}</span>
          </span>
          <ChevronRight :size="13" class="text-faint" />
        </button>
      </div>
    </div>

    <!-- Sidebar Footer with blended guitar.png -->
    <div class="sidebar-footer">
      <div id="mini-video-anchor" class="sidebar-guitar-card">
        <img
          src="/guitar.png"
          alt="Guitar"
          class="sidebar-guitar-img"
        />
        <div class="sidebar-guitar-overlay"></div>
        <div class="relative z-10 flex flex-col select-none">
          <span class="text-[12px] font-bold text-ink tracking-wide">Slide Up Music</span>
          <span class="text-[9.5px] text-faint">Pure Sound Experience</span>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar-nav-item {
  display: flex;
  height: 2.35rem;
  width: 100%;
  align-items: center;
  gap: 0.75rem;
  border-radius: var(--radius-sm, 8px);
  padding: 0 0.85rem;
  text-align: left;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  transition: all var(--t-fast);
}

.sidebar-nav-item:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sidebar-nav-item.active {
  background: var(--accent-purple) !important;
  color: #ffffff !important;
  font-weight: 600;
}

.quick-access-btn {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  border-radius: var(--radius-sm, 8px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-panel);
  padding: 0.4rem 0.65rem;
  width: 100%;
  text-align: left;
  cursor: pointer;
  transition: all var(--t-fast);
}

.quick-access-btn:hover {
  background: var(--bg-hover);
  border-color: var(--border-strong);
}

.sidebar-footer {
  margin-top: auto;
  padding: 0.5rem 0.65rem 0.65rem;
  width: 100%;
}

.sidebar-guitar-card {
  position: relative;
  width: 100%;
  height: 110px;
  border-radius: var(--radius-md, 12px);
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  background: linear-gradient(135deg, rgba(135, 63, 255, 0.15) 0%, rgba(40, 184, 255, 0.08) 100%);
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  padding: 0.65rem 0.75rem;
  isolation: isolate;
}

.sidebar-guitar-img {
  position: absolute;
  right: -8px;
  bottom: -6px;
  width: 115px;
  height: 115px;
  object-fit: contain;
  opacity: 0.85;
  pointer-events: none;
  transition: transform var(--t-fast) var(--ease), opacity var(--t-fast) var(--ease);
}

.sidebar-guitar-card:hover .sidebar-guitar-img {
  opacity: 1;
  transform: scale(1.06) rotate(-2deg);
}

.sidebar-guitar-overlay {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    to top,
    var(--bg-panel) 0%,
    transparent 60%
  );
  pointer-events: none;
  opacity: 0.75;
}
</style>
