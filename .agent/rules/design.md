# Design System: macOS Cupertino Terminal

## Identity & Aesthetic Direction
- **Theme**: macOS Ventura/Sonoma Cupertino aesthetic — restrained, dark-first, precision glassmorphism, authentic traffic lights, tight typography, zero superfluous clutter.
- **Surface Elevation**: Layered semi-transparency over Windows 11 DWM backdrop blur.

## Design Tokens

### Color Palette
- **Window Base Surface**: `rgba(18, 18, 18, 0.60)`
- **Title Bar Surface**: `rgba(28, 28, 32, 0.45)`
- **Window Border**: `rgba(255, 255, 255, 0.10)`
- **Interior Divider / Border**: `rgba(255, 255, 255, 0.08)`
- **Hover Overlay**: `rgba(255, 255, 255, 0.06)`
- **Active Overlay**: `rgba(255, 255, 255, 0.12)`

#### Typography Colors
- **Text Primary**: `#F5F5F7`
- **Text Secondary / Muted**: `#86868B`
- **Text Tertiary / Disabled**: `#55555B`

#### Window Controls (Minimalist Cupertino)
- **Minimalist Controls**: Sleek, understated monochrome controls positioned on the top-right (Minimize, Maximize, Close).
- **Control Styling**: Transparent background, subtle rounded hover state (`rgba(255, 255, 255, 0.08)`), icon color `#86868B` transitioning to `#F5F5F7` on hover. Close button hover accentuates to `#FF5F56`.
- **No Traffic Lights**: Authentic macOS colored circles omitted per user direction.

#### xterm.js Dark Palette (Curated Cupertino Terminal)
- **Background**: `transparent`
- **Foreground**: `#ECEFF4`
- **Cursor**: `#0A84FF`
- **Cursor Accent**: `#121212`
- **Selection Background**: `rgba(10, 132, 255, 0.30)`
- **Black**: `#1E1E22` | **Bright Black**: `#5A5D66`
- **Red**: `#FF6B6B` | **Bright Red**: `#FF8787`
- **Green**: `#32D74B` | **Bright Green**: `#4CD964`
- **Yellow**: `#FFD60A` | **Bright Yellow**: `#FFE043`
- **Blue**: `#0A84FF` | **Bright Blue**: `#409CFF`
- **Magenta**: `#BF5AF2` | **Bright Magenta**: `#DA8FFF`
- **Cyan**: `#64D2FF` | **Bright Cyan**: `#8BE0FF`
- **White**: `#E5E5EA` | **Bright White**: `#FFFFFF`

### Spacing Scale (8pt Grid)
- `space-1`: `4px`
- `space-2`: `8px`
- `space-3`: `16px`
- `space-4`: `24px`
- `space-5`: `32px`
- `space-6`: `48px`
- `space-7`: `64px`

### Border Radius Tiers
- **sm (`6px`)**: Small icon buttons, inline chips, status pills
- **md (`10px`)**: Tab elements, dropdown menus, terminal search bar
- **lg (`12px`)**: Main application window frame, modals, popovers
- **full (`9999px`)**: Traffic light dots (12px diameter circle), avatar badges, status indicators

### Blur & Glassmorphism
- **App Frame Filter**: `backdrop-filter: blur(30px); -webkit-backdrop-filter: blur(30px);`
- **Title Bar Filter**: `backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px);`
- **Tauri Window**: Transparent background (`transparent: true`), `decorations: false`, native shadow enabled.

### Typography
- **UI Font Stack**: `Inter, -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif`
- **Terminal Canvas Font**: `"JetBrains Mono Nerd Font", "MesloLGS NF", "SF Mono", monospace`
- **UI Type Scale**:
  - Title Bar Header: `12px`, font-weight 500 (Medium), tracking `-0.01em`
  - Tab Titles: `12px`, font-weight 400 (Regular)
  - Status / Metadata: `11px`, font-weight 400 (Regular)
  - Terminal Text: `13px` / `14px`, line-height `1.3`, font-weight 400

## Component Rules
1. **Title Bar**:
   - Height: `38px` fixed.
   - Window Controls: subtle, right-aligned minimalist icons (Minimize, Maximize, Close) with 6px rounded hover targets, preserving clean Cupertino aesthetic without traffic light dots.
2. **Window Sizing & Margins**:
   - Padding around xterm: `8px 12px 12px 12px` to give clean breathing room without wasting terminal real estate.
3. **No Decorative Clutter**:
   - Every border is 1px with controlled opacity. No drop shadows on buttons, no non-Apple gradients.
