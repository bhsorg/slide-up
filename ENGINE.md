libmpv + Tauri 2
- Frontend: Vue 3 + TypeScript
- Styling: Tailwind CSS
- Backend: Rust
- Playback: libmpv
- Integration: Native Windows rendering
- State: Pinia
- Icons: Lucide

src/
├── components/
│   ├── Sidebar.vue
│   ├── Topbar.vue
│   ├── VideoPlayer.vue
│   ├── AudioPlayer.vue
│   ├── PlaylistPanel.vue
│   └── PlaybackDock.vue
├── views/
│   ├── Home.vue
│   ├── MusicLibrary.vue
│   ├── VideoLibrary.vue
│   ├── Playlists.vue
│   └── Settings.vue
├── stores/
│   ├── player.ts
│   ├── library.ts
│   └── playlist.ts
├── composables/
├── services/
└── assets/