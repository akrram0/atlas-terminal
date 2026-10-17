# Design System: Atlas Terminal (Zeron-Inspired GPUI Native Architecture)

## Core Directive & Rules (MANDATORY)

1. **STRICT RULE: NO UNICODE ICONS**:
   - Under no circumstances should Unicode characters or emoji glyphs (e.g. `⚙`, `↺`, `✎`, `◐`, `⌕`, `×`, `⊞`, `⊟`) be used as UI icons.
   - All UI icons must be **native vector SVGs** rendered via GPUI's built-in `gpui::svg()` with an embedded `AssetSource`, identical to `https://github.com/zeronsh/zeron.git`.
   - Vector assets must follow the **Solar Icons (Linear weight)** aesthetic (clean 1.5px/1.75px stroked geometry, 24px coordinate space, uniform optical weight).

2. **INSPIRATION: ZERON (`https://github.com/zeronsh/zeron.git`)**:
   - Clean, flat, unified native control plane.
   - **No nested cards or awkward 8px perimeter margins** around the terminal grid. The terminal canvas runs edge-to-edge with the window, beneath a subtle chrome header bar.
   - **Restrained Chrome**: A single 36px top bar with minimal tabs (flat, quiet when inactive, crisp rounded rectangle when active) and tight, proportional window controls (38×30px, not bulky 46×36px).
   - **Whisper Breadcrumb**: Pane header is an integrated 26px breadcrumb bar carrying session status and micro-actions.

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
- `icons/window-minimize.svg`, `icons/window-maximize.svg`, `icons/window-restore.svg` — Window controls fallback
