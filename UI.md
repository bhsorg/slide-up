# 15. Unique UI Design Direction – Premium Media Player

The application must follow a distinctive, premium desktop media player design language. Use the provided visual reference as the primary design inspiration.

** See image ui ref public\slideup-ui.png

## Design Identity

Create an original visual identity. Do not imitate VLC, Spotify, Apple Music, Windows Media Player, or other existing media player interfaces.

The design should feel like a modern premium multimedia application with its own branding.

### Color Palette
- Primary background: Deep midnight navy (#080D1D).
- Secondary background: Dark indigo (#111831).
- Panel background: (#151D38).
- Primary accent: Electric purple (#873FFF).
- Secondary accent: Vibrant blue (#28B8FF).
- Highlight: Soft violet gradients.
- Primary text: (#F4F5FF).
- Secondary text: (#929BB8).
- Borders: Subtle indigo with low opacity.

Use purple-to-blue gradients selectively for active controls, selected navigation items, and important interactive elements.

### Application Layout

Build a desktop-first interface with four primary visual regions.

**1. Left Navigation Sidebar**
- Dedicated application logo and original branding.
- Home.
- Music Library.
- Video Library.
- Playlists.
- Favorites.
- Recently Played.
- Settings.
- Separate Quick Access section.
- Distinct active navigation state with a purple gradient background.
- Compact, elegant Lucide icons.

**2. Top Navigation Bar**
- Integrated media search field.
- Theme toggle.
- Window minimize, maximize and close controls.
- Minimal visual clutter.
- Consistent spacing and alignment.

**3. Main Media Workspace**

Use a spacious central media viewport.

For video playback:
- Large immersive video display.
- Rounded video container.
- Floating top-right controls.
- Quality indicator.
- Subtitle and fullscreen controls.
- Playback timeline integrated near the bottom.
- Auto-hide controls when the user is not interacting.

For audio playback:
- Show high-quality album artwork.
- Track information and metadata.
- Beautiful visual presentation without unnecessary visualizers.
- Keep the same overall application layout.

**4. Right Playlist Panel**
- Dedicated playlist header.
- Add and manage playlist actions.
- Compact media thumbnail.
- Track title and subtitle.
- Duration aligned to the right.
- Clear currently playing indicator.
- Selected track highlighted with a subtle purple gradient.
- Smooth scrolling for large playlists.

**5. Bottom Persistent Playback Dock**
- Current media artwork and information on the left.
- Favorite and queue actions.
- Previous, play/pause and next controls in the center.
- Large circular purple-gradient primary playback button.
- Playback progress and duration.
- Volume slider.
- Queue and additional playback controls.
- Subtle border and background separation from the main workspace.

### Visual Design Principles

- Use a consistent 8px spacing system.
- Use 12–18px corner radii for major panels.
- Use subtle shadows and restrained glow effects.
- Maintain strong visual hierarchy.
- Avoid excessive gradients and unnecessary glassmorphism.
- Use smooth transitions between active and inactive states.
- Use consistent icon sizes and stroke weights.
- Keep text readable and accessible.
- Avoid oversized typography.
- Ensure the interface remains visually balanced at different desktop resolutions.

### Interaction Design

Every visible control must have a meaningful interaction.

- Sidebar navigation must work.
- Search must filter actual media.
- Playlist selection must update playback.
- Playback controls must connect to the real playback engine.
- Volume and timeline controls must work.
- Fullscreen must work.
- Theme switching must work.
- Hover, focus, active and disabled states must be implemented.
- Tooltips should appear for icon-only controls.

### Responsive Desktop Behaviour

- Support different desktop window sizes.
- Sidebar should have a compact mode.
- Playlist panel should be collapsible.
- Main video viewport should resize proportionally.
- Bottom playback dock should remain accessible.
- Prevent overlapping controls and horizontal overflow.

### Branding & Originality

Create a unique application logo, icon treatment, and visual identity.

Do not reuse copyrighted logos, artwork, branding, or proprietary interface assets.

Use original CSS styling, Lucide icons, and appropriately licensed media artwork.

### Strict Design Requirements

1. Do not create a generic admin dashboard.
2. Do not use default HTML form styling.
3. Do not make the interface look like a web music streaming clone.
4. Do not overcrowd the playback screen.
5. Do not sacrifice usability for decorative effects.
6. Do not use placeholder controls.
7. Maintain consistent design across all application screens.
8. The final result must look and feel like a professionally designed native desktop media player.

**Final design objective:** A premium, immersive, dark multimedia desktop application with midnight navy surfaces, electric purple accents, vibrant blue highlights, an elegant navigation sidebar, a spacious media viewport, a dedicated playlist panel, and a persistent playback dock.