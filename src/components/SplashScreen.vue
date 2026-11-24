<script setup lang="ts">
import AppLogo from "./AppLogo.vue";

defineProps<{ visible: boolean }>();
</script>

<template>
  <Teleport to="body">
    <Transition name="splash">
      <div
        v-if="visible"
        class="fixed inset-0 z-[10000] flex flex-col items-center justify-center overflow-hidden bg-base text-ink"
        style="background-image: radial-gradient(circle at 50% 42%, rgba(135, 63, 255, 0.18), transparent 58%)"
      >
        <div class="splash-logo h-16 w-16 drop-shadow-[0_0_30px_rgba(135,63,255,0.45)]">
          <AppLogo />
        </div>

        <h1 class="mt-4 text-2xl font-semibold tracking-tight">Slide Up</h1>
        <p class="mt-1 text-[11px] tracking-[0.2em] text-muted uppercase">Play · Enjoy · Repeat</p>

        <div class="mt-6 flex items-center gap-1.5" aria-label="Loading">
          <span class="splash-dot" />
          <span class="splash-dot splash-dot-delay-1" />
          <span class="splash-dot splash-dot-delay-2" />
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.splash-logo {
  animation: splash-float 2.4s ease-in-out infinite;
}

.splash-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  background: var(--accent-purple-soft);
  animation: splash-pulse 1.1s ease-in-out infinite;
}

.splash-dot-delay-1 {
  animation-delay: 0.15s;
}

.splash-dot-delay-2 {
  animation-delay: 0.3s;
}

@keyframes splash-float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-6px); }
}

@keyframes splash-pulse {
  0%, 100% { opacity: 0.35; transform: scale(0.8); }
  50% { opacity: 1; transform: scale(1.25); }
}

.splash-enter-active,
.splash-leave-active {
  transition: opacity 420ms ease;
}

.splash-enter-from,
.splash-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .splash-logo,
  .splash-dot {
    animation: none;
  }
}
</style>
