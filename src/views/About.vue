<script setup lang="ts">
import { ref } from "vue";
import { Info, FileText, CheckCircle2, Cpu, ShieldCheck, Scale } from "@lucide/vue";

const activeSubTab = ref<"about" | "license" | "terms" | "privacy">("about");

const techStack = [
  { name: "Tauri 2", role: "Desktop Application Framework", license: "MIT / Apache-2.0" },
  { name: "Rust", role: "High-performance Core Backend", license: "MIT / Apache-2.0" },
  { name: "libmpv 2", role: "Hardware-accelerated Media Engine", license: "LGPL v2.1+" },
  { name: "Vue 3 & TypeScript", role: "Reactive User Interface", license: "MIT" },
  { name: "ONNX Runtime", role: "Offline Neural Inference Engine", license: "MIT" },
  { name: "Lucide Icons", role: "Icon System", license: "ISC" },
];

const features = [
  "GPU Hardware Accelerated Audio & Video Playback",
  "AI Video Enhancement with FSRCNNX Shader & Real-ESRGAN Upscaling",
  "Offline AI Captions via SenseVoice ONNX Inference",
  "Minimal Picture-in-Picture (PiP) Mode with Window Memory",
  "Windows Taskbar & System Tray Controls",
  "Precision Scrubbing with Storyboard Cached Frame Previews",
  "Bring-Your-Own M3U & HLS Live TV Streaming Player",
];
</script>

<template>
  <div class="about-container">
    <!-- Header -->
    <div class="page-header">
      <div class="page-heading">
        <div class="page-icon">
          <Info :size="22" />
        </div>
        <div>
          <h1>About, License & Policies</h1>
          <p>Application details, software licensing, terms of service, and privacy policy.</p>
        </div>
      </div>

      <!-- Compact Segmented Tab Controls -->
      <div class="tab-pill-group">
        <button
          type="button"
          class="tab-pill"
          :class="{ active: activeSubTab === 'about' }"
          @click="activeSubTab = 'about'"
        >
          <Info :size="13" />
          <span>About</span>
        </button>
        <button
          type="button"
          class="tab-pill"
          :class="{ active: activeSubTab === 'license' }"
          @click="activeSubTab = 'license'"
        >
          <FileText :size="13" />
          <span>License</span>
        </button>
        <button
          type="button"
          class="tab-pill"
          :class="{ active: activeSubTab === 'terms' }"
          @click="activeSubTab = 'terms'"
        >
          <Scale :size="13" />
          <span>Terms</span>
        </button>
        <button
          type="button"
          class="tab-pill"
          :class="{ active: activeSubTab === 'privacy' }"
          @click="activeSubTab = 'privacy'"
        >
          <ShieldCheck :size="13" />
          <span>Privacy</span>
        </button>
      </div>
    </div>

    <!-- Content Body -->
    <div class="about-body">
      <!-- Tab 1: About -->
      <div v-if="activeSubTab === 'about'" class="about-cards-grid">
        <!-- App Overview Card -->
        <div class="su-card hero-card">
          <div class="hero-header">
            <div class="app-avatar">
              <Cpu :size="26" />
            </div>
            <div>
              <h2 class="app-title">Slide Up Desktop</h2>
              <div class="app-meta">
                <span class="version-tag">v1.0.0</span>
                <span class="meta-dot">•</span>
                <span class="meta-text">64-bit Windows</span>
                <span class="meta-dot">•</span>
                <span class="meta-badge">Production Ready</span>
              </div>
            </div>
          </div>
          <p class="hero-desc">
            A high-performance modern desktop media player engineered for seamless audio and video playback,
            powered by native Rust, libmpv, and AI-accelerated shaders.
          </p>
        </div>

        <!-- Highlights & Features Card -->
        <div class="su-card">
          <h3 class="card-title">Key Capabilities</h3>
          <ul class="features-list">
            <li v-for="(feat, idx) in features" :key="idx" class="feature-item">
              <CheckCircle2 :size="15" class="feature-icon" />
              <span>{{ feat }}</span>
            </li>
          </ul>
        </div>

        <!-- Tech Stack Card -->
        <div class="su-card full-width">
          <h3 class="card-title">Architecture & Dependencies</h3>
          <div class="tech-table">
            <div class="tech-row tech-head">
              <span>Component</span>
              <span>Role</span>
              <span>License</span>
            </div>
            <div v-for="item in techStack" :key="item.name" class="tech-row">
              <span class="font-semibold text-ink">{{ item.name }}</span>
              <span class="text-muted">{{ item.role }}</span>
              <span class="tech-license-tag">{{ item.license }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Tab 2: License -->
      <div v-else-if="activeSubTab === 'license'" class="license-wrapper space-y-4">
        <div class="su-card">
          <div class="flex items-center gap-2 mb-3">
            <ShieldCheck :size="18" class="text-purple-400" />
            <h3 class="card-title mb-0">Open Source Licensing (MIT)</h3>
          </div>

          <div class="license-box">
            <p class="font-bold text-ink text-sm">MIT License</p>
            <p class="mt-1 text-faint text-xs font-mono">Copyright (c) 2026 Slide Up Contributors</p>
            <p class="mt-3 text-ink text-xs leading-relaxed">
              Permission is hereby granted, free of charge, to any person obtaining a copy
              of this software and associated documentation files (the "Software"), to deal
              in the Software without restriction, including without limitation the rights
              to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
              copies of the Software, and to permit persons to whom the Software is
              furnished to do so, subject to the following conditions:
            </p>
            <p class="mt-2 text-ink text-xs leading-relaxed">
              The above copyright notice and this permission notice shall be included in all
              copies or substantial portions of the Software.
            </p>
            <p class="mt-2 text-muted text-xs leading-relaxed uppercase">
              THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
              IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
              FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
              AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
              LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
              OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
              SOFTWARE.
            </p>
          </div>
        </div>

        <div class="su-card">
          <div class="flex items-center gap-2 mb-3">
            <FileText :size="18" class="text-purple-400" />
            <h3 class="card-title mb-0">Third-Party Software Acknowledgements</h3>
          </div>
          <div class="legal-content">
            <p>
              Slide Up is built upon reputable open-source projects. Each dependency retains its own
              respective open-source licensing agreement:
            </p>
            <ul class="legal-list">
              <li><strong class="text-ink">libmpv</strong> — GNU Lesser General Public License (LGPL) v2.1 or later.</li>
              <li><strong class="text-ink">Tauri Framework</strong> — Apache-2.0 or MIT License.</li>
              <li><strong class="text-ink">Vue 3 & Vite</strong> — MIT License.</li>
              <li><strong class="text-ink">ONNX Runtime</strong> — MIT License.</li>
              <li><strong class="text-ink">SenseVoice & Real-ESRGAN</strong> — Respective open research & BSD/Apache licenses.</li>
              <li><strong class="text-ink">Lucide Icons</strong> — ISC License.</li>
            </ul>
          </div>
        </div>
      </div>

      <!-- Tab 3: Terms of Service -->
      <div v-else-if="activeSubTab === 'terms'" class="license-wrapper space-y-4">
        <div class="su-card">
          <div class="flex items-center gap-2 mb-3">
            <Scale :size="18" class="text-purple-400" />
            <h3 class="card-title mb-0">Terms of Service & Usage Agreement</h3>
          </div>

          <div class="legal-content space-y-4">
            <section class="legal-section">
              <h4 class="legal-section-title">1. Acceptance of Terms</h4>
              <p>
                By downloading, installing, or using Slide Up Desktop ("the Application"), you agree to be bound by these
                Terms of Service. If you do not agree to these terms, do not install or use the Application.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">2. Nature of the Application (Player Tool Only)</h4>
              <p>
                Slide Up is strictly a multimedia player utility designed to render locally stored media files and network streams
                (HLS, M3U8, RTSP, RTMP). Slide Up does not host, curate, broadcast, sell, or bundle any television channels, video
                streams, audio tracks, or copyrighted content.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">3. User Responsibility & Content Legality</h4>
              <p>
                Users are solely responsible for all content, playlists (M3U), stream links, and media files they import or play using
                the Application. You warrant and represent that you have acquired legitimate authorization, license, or legal rights to
                access and play any media streams or files you choose to load into Slide Up.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">4. Copyright Infringement & DMCA Stance</h4>
              <p>
                Slide Up strongly respects the intellectual property rights of content creators. The Application does not contain, facilitate,
                or endorse unauthorized distribution of copyrighted material. Any use of the Application to access protected or pirated
                streams is strictly prohibited by our usage policy and is solely the legal liability of the end user.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">5. Disclaimer of Warranties & Limitation of Liability</h4>
              <p>
                The Application is provided "as is" and "as available". The developers disclaim all warranties, express or implied.
                In no event shall the developers or distributors be liable for any direct, indirect, incidental, or consequential damages
                arising out of the use or inability to use the Application.
              </p>
            </section>
          </div>
        </div>
      </div>

      <!-- Tab 4: Privacy Policy -->
      <div v-else-if="activeSubTab === 'privacy'" class="license-wrapper space-y-4">
        <div class="su-card">
          <div class="flex items-center gap-2 mb-3">
            <ShieldCheck :size="18" class="text-emerald-400" />
            <h3 class="card-title mb-0">Privacy Policy (Microsoft Store Compliant)</h3>
          </div>

          <div class="legal-content space-y-4">
            <div class="legal-highlight-box">
              <strong class="text-emerald-300">Summary:</strong> Slide Up operates on a strict <strong>Zero Data Collection</strong> policy.
              We do not collect, monitor, track, store, or sell any personal data, media habits, or usage telemetry.
            </div>

            <section class="legal-section">
              <h4 class="legal-section-title">1. Information We Do Not Collect</h4>
              <ul class="legal-list">
                <li>No personal identification information (names, emails, addresses, phone numbers).</li>
                <li>No playback telemetry, watched media logs, or browsing habits.</li>
                <li>No device identifiers, IP tracking, or user fingerprinting.</li>
                <li>No third-party advertising SDKs or tracking cookies.</li>
              </ul>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">2. 100% Offline Local Processing</h4>
              <p>
                All smart AI features — including real-time offline speech-to-text captions (SenseVoice ONNX) and video enhancement
                (Real-ESRGAN and shaders) — execute 100% locally on your PC's hardware. No media, audio frames, or text transcripts are
                ever transmitted to external cloud servers.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">3. Network Communication</h4>
              <p>
                The Application only initiates outbound network connections when explicitly instructed by the user, specifically:
              </p>
              <ul class="legal-list">
                <li>Connecting to user-provided M3U/HLS stream URLs during Live TV playback.</li>
                <li>Fetching synchronized lyrics from public APIs when online lyrics search is requested by the user.</li>
              </ul>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">4. Local Device Storage</h4>
              <p>
                Application configuration (volume preferences, audio/video equalizer settings, custom playlist URLs, and local library file paths)
                is stored exclusively on your local device (in app localStorage and SQLite database). You may reset or purge this data at any time
                via the Settings page.
              </p>
            </section>

            <section class="legal-section">
              <h4 class="legal-section-title">5. Contact & Updates</h4>
              <p>
                We may periodically update this policy to reflect application enhancements. All modifications will be updated within this page.
              </p>
            </section>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.about-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 1.25rem 1.5rem;
  overflow-y: auto;
  gap: 1.15rem;
  background: var(--bg-base);
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-shrink: 0;
}

.page-heading {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.page-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 2.5rem;
  height: 2.5rem;
  border-radius: 0.65rem;
  background: var(--accent-gradient, linear-gradient(135deg, #8b5cf6, #6d28d9));
  color: #fff;
  flex-shrink: 0;
}

.page-heading h1 {
  font-size: 1.25rem;
  font-weight: 700;
  margin: 0;
  color: var(--text-primary);
}

.page-heading p {
  font-size: 0.78rem;
  color: var(--text-secondary);
  margin: 0.15rem 0 0;
}

/* Compact Segmented Tabs */
.tab-pill-group {
  display: inline-flex;
  align-items: center;
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  box-shadow: var(--shadow-panel);
  border-radius: 8px;
  padding: 3px;
  gap: 3px;
  flex-shrink: 0;
}

.tab-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.25rem 0.65rem;
  height: 28px;
  border-radius: 6px;
  font-size: 0.76rem;
  font-weight: 500;
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.tab-pill:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.tab-pill.active {
  background: var(--accent-purple, #8b5cf6);
  color: #ffffff !important;
  font-weight: 600;
  box-shadow: 0 2px 8px rgba(139, 92, 246, 0.45);
}

.about-body {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.about-cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: 1rem;
}

.su-card {
  background: var(--bg-panel);
  border: 1px solid var(--border-subtle);
  border-radius: 0.75rem;
  padding: 1.25rem;
  box-shadow: var(--shadow-panel);
}

.full-width {
  grid-column: 1 / -1;
}

.hero-card {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.hero-header {
  display: flex;
  align-items: center;
  gap: 0.85rem;
}

.app-avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 3rem;
  height: 3rem;
  border-radius: 0.65rem;
  background: rgba(139, 92, 246, 0.18);
  color: var(--accent-purple, #8b5cf6);
  border: 1px solid rgba(139, 92, 246, 0.35);
}

.app-title {
  font-size: 1.15rem;
  font-weight: 700;
  margin: 0;
  color: var(--text-primary);
}

.app-meta {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.2rem;
  font-size: 0.72rem;
}

.version-tag {
  font-weight: 600;
  color: var(--accent-purple, #8b5cf6);
}

.meta-dot {
  color: var(--text-muted);
}

.meta-text {
  color: var(--text-secondary);
}

.meta-badge {
  background: rgba(16, 185, 129, 0.18);
  color: #10b981;
  padding: 0.1rem 0.45rem;
  border-radius: 0.25rem;
  font-weight: 600;
}

.hero-desc {
  font-size: 0.84rem;
  color: var(--text-primary);
  line-height: 1.6;
  margin: 0;
}

.card-title {
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 0.75rem;
}

.features-list {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.feature-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.82rem;
  color: var(--text-primary);
}

.feature-icon {
  color: #10b981;
  flex-shrink: 0;
}

.tech-table {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.tech-row {
  display: grid;
  grid-template-columns: 180px 1fr 140px;
  align-items: center;
  padding: 0.45rem 0.6rem;
  border-radius: 0.4rem;
  font-size: 0.78rem;
}

.tech-head {
  font-weight: 600;
  color: var(--text-muted);
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: 0.5rem;
}

.tech-row:not(.tech-head):hover {
  background: var(--bg-hover);
}

.tech-license-tag {
  display: inline-block;
  font-size: 0.7rem;
  color: var(--accent-purple, #8b5cf6);
  font-family: monospace;
}

/* Legal text styling - high contrast, theme-aware */
.legal-content p,
.legal-content li,
.license-box p {
  color: var(--text-primary);
  font-size: 0.83rem;
  line-height: 1.65;
}

.legal-section-title {
  color: var(--text-primary);
  font-weight: 600;
  font-size: 0.88rem;
  margin-bottom: 0.35rem;
}

.legal-list {
  list-style-type: disc;
  padding-left: 1.25rem;
  margin-top: 0.4rem;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.legal-list li {
  color: var(--text-secondary);
  font-size: 0.82rem;
}

.license-box {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: 0.5rem;
  padding: 1.15rem;
}

.legal-highlight-box {
  background: rgba(16, 185, 129, 0.12);
  border: 1px solid rgba(16, 185, 129, 0.35);
  color: var(--text-primary);
  padding: 0.75rem 1rem;
  border-radius: 0.5rem;
  font-size: 0.82rem;
  line-height: 1.6;
}
</style>
