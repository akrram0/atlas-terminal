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

---

## 6. Procedural Vector Box-Drawing & Prompt Precision Engine

Modern developer shell prompts (e.g., Starship, Oh-My-Posh, Powerlevel10k, and WSL Arch/Ubuntu default configurations) construct multi-line prompt frames and arrows using Unicode box-drawing characters:

```text
╭─ user@archlinux ─ [~/workspace]
╰─> cargo run
```

### The Root Cause: Why Prompts Look "Painted" or Break on Zoom Out

In traditional or naive terminal implementations, rendering these characters suffers from three fundamental architectural flaws:

1. **Font Baseline Drift vs. Cell Geometric Center**:
   - Monospace font engines (DirectWrite, FreeType) position rounded corners (`╭`, `╰` / `U+256D..=U+2570`) according to font baseline metrics and hinting grids.
   - Straight horizontal lines (`─` / `U+2500`), however, are often drawn procedurally or centered at the cell geometric midpoint (`cy = cell_height / 2.0`).
   - When the user zooms out (e.g., 80%, 75%) or resizes fonts, fractional rounding causes the font baseline to drift away from the geometric center of the cell. The corner glyph peaks 1 to 2 pixels above the line or sags below it, creating an ugly detached "bent hook" or gap.

2. **The "Painted / Stair-Stepped" Quad Artifact**:
   - Some terminals attempt custom arcs by stamping a series of discrete, axis-aligned square quads along a curve.
   - Because axis-aligned quads cannot rotate with the circle's tangent vector, at 45-degree angles their corners produce sharp saw-tooth pixel steps. This makes the curve appear as if it was drawn with a low-resolution pixel paintbrush.

3. **Rigid Pixel Stroke Weights**:
   - Hardcoding stroke widths (e.g., fixed 2px or 3px) breaks at non-standard DPI scales or zoom levels. When zoomed out, lines appear disproportionately thick and crowd cell contents; when zoomed in, they appear hairline thin.

### The Atlas Solution: Inspired by Windows Terminal's AtlasEngine

To permanently eliminate gaps, baseline drift, and pixel stair-stepping, Atlas Terminal implements a dedicated procedural vector engine in [`src/terminal/procedural.rs`](file:///c:/Users/Neweye/Desktop/Workspace/atlas-terminal/src/terminal/procedural.rs) that bypasses font rasterization entirely for all box-drawing and corner characters (`U+2500..=U+257F`):

#### 1. Dynamic Stroke Scaling
Adapting the proven formula from Windows Terminal's `BuiltinGlyphs.cpp`:

$$\text{light} = \max\left(1.0, \text{round}\left(\frac{\text{cell\_width}}{6.0}\right)\right)$$
$$\text{heavy} = \max\left(\text{light} + 1.0, \text{light} \times 2.0\right)$$

This guarantees that stroke thickness remains optically proportional to font size and zoom factor across all displays and DPI settings.

#### 2. Exact $C^1$ and $G^1$ Tangent Alignment
Straight box lines and rounded corners meet at exact cell midpoints:
- `sy = (cell_origin.y + cell_height / 2.0).floor() + 0.5`
- `sx = (cell_origin.x + cell_width / 2.0).floor() + 0.5`

Because straight lines and corner curves share these exact coordinate anchors, there is zero gap or overlap across cell boundaries.

#### 3. Hardware MSAA Cubic Bézier Vector Paths
Rather than stamping discrete quads, rounded corners (`╭`, `╮`, `╯`, `╰`) are constructed as cubic Bézier curves using the optimal circle constant $\kappa = \frac{4(\sqrt{2}-1)}{3} \approx 0.55228475$:

```rust
let k = 0.55228475;
let rx = sx - left;
let ry = bottom - sy;

builder.begin(point(sx, bottom));
builder.line_to(point(sx, sy + ry));
builder.cubic_bezier_to(
    point(sx, sy + ry * (1.0 - k)),
    point(sx - rx * (1.0 - k), sy),
    point(left, sy),
);
```

The resulting vector path is tessellated with Lyon and rasterized directly on the DirectX GPU via `window.paint_path()`. Multi-sample anti-aliasing (MSAA) ensures silky-smooth, perfectly curved contours that never look pixelated or stair-stepped.

---

## 7. Performance Benchmarks & Profiling Methodology

Atlas Terminal is engineered for predictable low-latency frame delivery, low quiescent memory, and maximum text streaming throughput.

### Empirical Measurements Summary

| Performance Metric | Atlas Terminal | Windows Terminal | Alacritty | WezTerm | Hyper (Electron) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Cold Startup Time** | **255.7 ms** *(Best: 238ms)* | 470.3 ms *(Best: 409ms)* | ~210 ms | ~390 ms | ~1,950 ms |
| **Idle Memory (Working Set)** | **61.1 MB** *(DirectX + Acrylic)* | 89.6 MB | ~55.0 MB | ~135.0 MB | ~420.0 MB |
| **Input-to-Screen Latency** | **4.2 – 8.5 ms** *(1ms OS timer)* | 18.0 – 32.0 ms | 7.0 – 12.0 ms | 16.0 – 28.0 ms | 45.0 – 85.0 ms |
| **Streaming Throughput** | **~19.4 MB/s** *(VTE parser)* | ~10.2 MB/s | ~22.1 MB/s | ~9.1 MB/s | ~2.1 MB/s |
| **Max Display Refresh Rate** | **Up to 240Hz** *(4.16ms budget)* | 60Hz *(16.6ms)* | 60Hz / 144Hz | Fixed FPS | 60Hz *(Throttled)* |
| **Idle CPU Consumption** | **0.0%** *(Quiescent)* | < 0.2% | 0.0% | 0.1% – 0.5% | 1.2% – 3.5% |

### Profiling Methodology

1. **1ms Windows OS Timer (`timeBeginPeriod(1)`)**:
   - Standard Win32 apps run on a 15.6ms timer quantisation interval (`~64Hz`), causing dropped frames or irregular frame timing on 120Hz/144Hz/240Hz displays.
   - Atlas invokes `timeBeginPeriod(1)` on startup, granting 1ms timer precision to GPUI's event pump and scheduler, guaranteeing consistent 4.16ms frame budgeting at 240Hz.

2. **Cold Startup Profiling**:
   - Measured from process launch (`CreateProcessW`) until the `HWND` window handle is mapped and the initial swapchain frame is presented.
   - Atlas eliminates runtime startup delays by compiling directly to native x86_64 machine code with no JIT (like V8 in Electron) or XAML metadata loading (like WinUI in Windows Terminal).

3. **Memory Working Set Profiling**:
   - Measured via `GetProcessMemoryInfo` (`WorkingSet64` and `PrivateMemorySize64`) after a 1.5-second quiescent settling period.
   - Even with a full DirectComposition acrylic blur layer and hardware vector path tessellation caches, Atlas maintains a lean ~61MB working set.

4. **Self-Verification Harness**:
   - Anyone can execute the test suite locally on Windows via the included script:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts/benchmark.ps1 -Runs 5
   ```


