# Atlas Terminal — Architecture & Technical Design

This document details the internal architecture, rendering pipeline, and platform integrations of **Atlas Terminal**.

---

## 1. High-Level Architecture

Atlas Terminal is built from the ground up in Rust, combining three primary subsystems:

```
┌────────────────────────────────────────────────────────────────────────┐
│                          AtlasView (GPUI App)                          │
│                                                                        │
│   ┌─────────────────────┐   ┌──────────────────────────────────────┐   │
│   │ Top Bar & Chrome    │   │ Overlays (Palette, Settings, Modals) │   │
│   └─────────────────────┘   └──────────────────────────────────────┘   │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │            TerminalElement (GPU Quad & Text Canvas)            │   │
│   └────────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ DirectComposition / DirectX
┌───────────────────────────────────▼────────────────────────────────────┐
│                  Platform Layer (src/platform/windows.rs)              │
│  - Subclassed Win32 WndProc (Strip WS_CAPTION, Keep WS_THICKFRAME)     │
│  - DWM Acrylic Composition (SetWindowCompositionAttribute)             │
│  - DWM Rounded Corners (DWMWA_WINDOW_CORNER_PREFERENCE = ROUND)        │
│  - WM_NCCALCSIZE zero client insets + custom WM_NCHITTEST drag band    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ IPC / PTY
┌───────────────────────────────────▼────────────────────────────────────┐
│                    Terminal Session (src/session.rs)                   │
│  - alacritty_terminal VTE & Grid State Machine                         │
│  - Windows ConPTY / WSL2 Pseudoterminal Pipe                           │
│  - Inline OSC 1337 Image Interception & Anchor Calculation             │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Rendering Pipeline & DirectX GPU Acceleration

Instead of using WebViews, HTML DOMs, or software CPU rasterizers, Atlas Terminal uses **GPUI 0.2.2** (Zed's UI framework) to compile rendering operations into hardware DirectX quads:

1. **Prepaint Phase**:
   - Shapes text using system font resolvers (`DirectWrite` on Windows).
   - Measures exact monospace character advances (`cell_width`, `cell_height`).
   - Resizes the PTY grid dimensions (`cols`, `rows`) to match the client window geometry.
2. **Paint Phase**:
   - **Background Quads**: Aggregates runs of contiguous cells sharing the same background color into unified geometric quads to minimize draw calls.
   - **Corner Rounding**: Any quads touching the bottom row and outer side edges apply a `13.0px` corner radius, preventing child fills from bleeding past the rounded window frame.
   - **Inline Images**: Decoded into GPU textures and blitted with hardware filtering at exact cell grid line offsets.
   - **Kinetic Cursor**: Paints a floating animated quad using Apple spring deceleration physics (`dx * (1 - e^(-28 * dt))`).
   - **Text Glyphs**: Renders shaped glyphs on top of background and cursor layers.

---

## 3. Windows 11 DWM Integration & Acrylic Blur

Standard Win32 windows with custom chrome either suffer from thick non-client borders or lose Windows 11 rounded corners and shadows. Atlas Terminal solves this via runtime subclassing:

1. **Frameless Full-Bleed Client Area**:
   - Strips `WS_CAPTION` while retaining `WS_THICKFRAME`.
   - Handles `WM_NCCALCSIZE` and returns `0`, giving the client area 100% full-bleed window coverage.
   - Coordinates `WM_GETMINMAXINFO` to keep maximized windows pinned to the active monitor's work area.
2. **Native Rounded Corners**:
   - Explicitly sets `DWMWA_WINDOW_CORNER_PREFERENCE` to `DWMWCP_ROUND (2)`.
   - Removes the DWM 1px outline via `DWMWA_BORDER_COLOR = DWMWA_COLOR_NONE`.
3. **Hardware Acrylic Blur Behind Transparent Swapchain**:
   - Applies `SetWindowCompositionAttribute` with `ACCENT_ENABLE_ACRYLICBLURBEHIND (4)`.
   - The root GPUI window operates on a transparent DirectX swapchain (`WindowBackgroundAppearance::Transparent`).
   - Empty terminal cells skip background quad generation, letting the DWM acrylic blur shine through with zero CPU overhead.

---

## 4. Inline Image Graphics Architecture (OSC 1337)

Unlike Sixel or Kitty graphics protocols that require complex terminal modes, Atlas implements a clean, robust parser for the iTerm2 OSC 1337 standard:

- **State Machine Interception**: Intercepts `\x1b]1337;File=...:\x07` escape streams before they reach ConPTY.
- **Base64 Async Decoding**: Decodes binary payloads off the UI thread and constructs GPU `Image` assets.
- **Scrollback & Reflow Anchoring**: Anchor positions are calculated relative to absolute terminal history size (`anchor_line = history + cursor_line`), ensuring graphics stay 1:1 locked with companion text during window reflow, font zoom, and scrollback.

---

## 5. Kinetic Cursor Physics

The terminal cursor in Atlas Terminal is mathematically modeled as a damped spring:

$$\Delta x = (x_{\text{target}} - x_{\text{current}}) \times (1 - e^{-28 \Delta t})$$

- Glides smoothly when typing or moving across lines within a 15-column / 10-row proximity window.
- Snaps instantly across large jumps (e.g., screen clears or page jumps) to eliminate visual disorientation.
- Features continuous organic breathing alpha when idle (`0.25 + 0.75 * (sin(phase) * 0.5 + 0.5)`), locking to 100% solid opacity during active typing.
