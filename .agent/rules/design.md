# Design System: Atlas Terminal (Zeron-Inspired GPUI Native Architecture)

## Core Directive & Rules (MANDATORY)

1. **STRICT RULE: NO UNICODE ICONS**:
   - Under no circumstances should Unicode characters or emoji glyphs (e.g. `⚙`, `↺`, `✎`, `◐`, `⌕`, `×`, `⊞`, `⊟`) be used as UI icons.
   - All UI icons must be **native vector SVGs** rendered via GPUI's built-in `gpui::svg()` with an embedded `AssetSource`, identical to `https://github.com/zeronsh/zeron.git`.
   - Vector assets must follow the **Solar Icons (Linear weight)** aesthetic (clean 1.5px/1.75px stroked geometry, 24px coordinate space, uniform optical weight).

2. **INSPIRATION: ZERON & GHOSTTY (`https://github.com/zeronsh/zeron.git`)**:
   - Clean, flat, unified native control plane.
   - **No nested cards or awkward 8px perimeter margins** around the terminal grid. The terminal canvas runs edge-to-edge with the window, beneath a subtle chrome header bar.
   - **Restrained Chrome**: A single unified 36px top bar with minimal tabs (flat, quiet when inactive, crisp rounded rectangle with process icon + title when active) and tight, proportional window controls (38×30px, not bulky 46×36px).
   - **No Redundant Secondary Bars**: No second pane header bar. Process titles and icons live on the active tab pill in the top bar. Pane splitting and management rely entirely on keyboard shortcuts (`Ctrl+Shift+D` / `Ctrl+Shift+E` / `Ctrl+Shift+W` / `Ctrl+Shift+M`) and the Command Palette (`Ctrl+Shift+P`).

---

## Design Tokens

### Color Palette (Curated Dual Theme)

#### 1. Paper / Warm (Default — Inspired by Zed & Claude Code)
- **Terminal Surface (`bg`)**: `#EDE8DC` (warm cream)
- **Chrome / Top Bar (`panel_bg`)**: `#E5E0D4`
- **Active Tab / Elevated Surface (`elevated_bg`)**: `#F5F0E8`
- **Elevated Border**: `rgba(0, 0, 0, 0.08)`
- **Hairline Dividers**: `rgba(0, 0, 0, 0.08)`
- **Primary Text (`fg`, `chrome_fg_active`)**: `#1C1A16`
- **Muted Text (`chrome_fg`)**: `#7A7570`
- **Faint / Shortcuts (`faint_fg`)**: `#9A9590`
- **Accent Brand**: `#C05A28` (warm terracotta rust)
- **Cursor**: `#C05A28`

#### 2. Dark (Inspired by Zeron / OpenCode Dark)
- **Terminal Surface (`bg`)**: `#0E0E0D` (matte near-black)
- **Chrome / Top Bar (`panel_bg`)**: `#0A0A09`
- **Active Tab / Elevated Surface (`elevated_bg`)**: `#161614`
- **Elevated Border**: `rgba(255, 255, 255, 0.08)`
- **Hairline Dividers**: `rgba(255, 255, 255, 0.06)`
- **Primary Text (`fg`, `chrome_fg_active`)**: `#E8E4DC`
- **Muted Text (`chrome_fg`)**: `#6B6860`
- **Faint / Shortcuts (`faint_fg`)**: `#4A463F`
- **Accent Brand**: `#E08A62`
- **Cursor**: `#D08A5C`

---

## Icon Architecture (`gpui::svg`)

### Pipeline
- All SVGs are embedded into the binary using `include_bytes!` inside an `AssetSource` implementation.
- Loaded into GPUI via `Application::new().with_assets(IconsAssets)`.
- Rendered via a helper:
  ```rust
  pub fn icon(path: &'static str) -> gpui::Svg {
      gpui::svg().path(path).flex_none()
  }
  ```
- Tints are applied via `.text_color(color)` directly on the SVG element.

### Asset Set (Solar Linear & Agent Brands)
- `icons/terminal.svg` — Terminal session
- `icons/plus.svg` — New tab / action
- `icons/close.svg` — Close tab / pane
- `icons/split-vertical.svg` & `icons/split-horizontal.svg` — Panes
- `icons/settings.svg` — Settings gear / sliders
- `icons/magnifer.svg` — Search / command palette
- `icons/pen.svg` — Edit / rename tab
- `icons/refresh.svg` — Restart session
- `icons/sun.svg` / `icons/moon.svg` — Theme toggle
- `icons/folder.svg` — Inactive directory / workspace
- `icons/claude-mark.svg`, `icons/opencode-mark.svg`, `icons/ubuntu.svg` — Agent & runtime brands
- `icons/arch.svg`, `icons/debian.svg`, `icons/fedora.svg`, `icons/kali.svg`, `icons/alpine.svg`, `icons/opensuse.svg`, `icons/linux-mint.svg`, `icons/nixos.svg`, `icons/pop-os.svg`, `icons/rocky.svg`, `icons/alma.svg`, `icons/gentoo.svg`, `icons/docker.svg`, `icons/linux.svg` — WSL distro marks, selected from the detected default distribution and tinted by `currentColor`
- `icons/window-minimize.svg`, `icons/window-maximize.svg`, `icons/window-restore.svg` — Window controls fallback

---

## Window Architecture & Geometry Scale

### 1. Window Framing & Transparency
- **Window Backdrop**: Fully transparent (`WindowBackgroundAppearance::Transparent` + `WindowDecorations::Client`). No opaque system canvas clearing.
- **Root Element**: Must NEVER paint a solid background quad (no `bg(...)` on root) to avoid re-introducing sharp 90-degree corners.
- **Main Window Container**: A single unified container wrapping the top tab bar and terminal sheet with:
  - `.rounded(px(14.0))` (when restored) / `0px` (when maximized)
  - `.border_1()` with subtle border `.border_color(rgba(0xFFFFFF1A))`
  - `.bg(theme.panel_bg)`
  - `.overflow_hidden()`
  - `.shadow_lg()` for soft glass depth

### 2. Corner Radius & Gap Elimination Formula
- **Outer Radius**: `14.0px`
- **Inner Radius Formula**: `inner_radius = outer_radius - border_width` = `13.0px`
- **Top Bar (`bar`)**: Styled with `.rounded_t(px(13.0))` with matching `bg(theme.panel_bg)`.
- **Close Button**: Rightmost control styled with `.rounded_tr(px(13.0))` so red hover highlights curve naturally with the window.
- **Terminal Sheet & Cells (`terminal-sheet`, `pane-cell`)**: Mounts directly beneath the top bar, styled with `.rounded_b(px(13.0))` and matching `bg(theme.panel_bg)`. No secondary pane header strips.

### 3. Safe Insets & Grid Padding
- **Cell Padding**: Exactly 10px safe padding (`.px(px(10.0)).py(px(10.0))`) between the container border and the terminal cell grid so text and prompts never touch or clip against the rounded corners.

### 4. Quad Transparency & Anti-Leak Rules
- **GPUI Scissor Limitation**: GPUI's `overflow_hidden()` clips exclusively to the element's bounding rectangle, NOT the rounded border path. Background quads drawn by child elements will poke through rounded corners if not properly managed.
- **No Default BG Quads**: In `TerminalElement::paint`, do not paint a solid background quad for cells matching the default background. The parent container's rounded background must show through.
- **Edge Non-Default Quads**: Any non-default background spans (prompts, selections, reverse-video) touching the bottom row and side edges must explicitly pass `corner_radii: Corners { bottom_left: px(13.0), bottom_right: px(13.0) }` to `fill(...)`.
- **No Layered Contrasting Backgrounds**: Never wrap the terminal grid in intermediate full-size `div` elements with differing background colors (`sheet_bg`, etc.). All container and wrapper surfaces must share `theme.panel_bg`.

