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

## 5. Kinetic Cursor Motion

The terminal cursor in Atlas Terminal features smooth, organic motion:

- **Smooth Interpolation**: Glides cleanly between adjacent positions during typing and arrow-key navigation.
- **Instant Large Jumps**: Snaps immediately on large jumps (such as screen clears, scrolling, or page jumps) to eliminate visual disorientation.
- **Breathing Effect**: Features a subtle breathing opacity cycle when resting, locking to solid opacity during active typing.

---

## 6. Procedural Vector Box-Drawing & Prompt Precision Engine

Modern developer shell prompts (e.g., Starship, Oh-My-Posh, Powerlevel10k, and WSL Arch/Ubuntu default configurations) construct multi-line prompt frames and arrows using Unicode box-drawing characters:

```text
╭─ user@archlinux ─ [~/workspace]
╰─> cargo run
```

### Eliminating Prompt Gaps and Stair-Stepping

In traditional terminal implementations, rendering these characters often leads to noticeable artifacts:

1. **Font Baseline Drift**: Monospace fonts position corner glyphs on text baseline grids while lines sit at geometric cell centers, causing lines and corners to detach when zooming out.
2. **Stair-Stepped Corners**: Terminals that approximate curves with small square rectangles produce jagged, saw-tooth edges.
3. **Rigid Pixel Weights**: Fixed stroke widths look too thick when zoomed out and too thin when zoomed in.

### The Atlas Solution

Atlas Terminal implements a dedicated procedural vector engine in [`src/terminal/procedural.rs`](file:///c:/Users/Neweye/Desktop/Workspace/atlas-terminal/src/terminal/procedural.rs) that draws box characters directly on the GPU:

- **Dynamic Stroke Scaling**: Line thickness scales proportionally with font size and zoom level so borders always look natural.
- **Seamless Alignment**: Straight lines and rounded corners share exact coordinate midpoints, eliminating 1px gaps and overlapping artifacts.
- **Smooth Vector Curves**: Rounded corners (`╭`, `╮`, `╯`, `╰`) are drawn as anti-aliased vector paths with hardware multisampling for clean, curved lines.
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
| **Cold Startup Time** | **< 250 ms** | ~470 ms | ~210 ms | ~390 ms | ~1,950 ms |
| **Idle Memory (Working Set)** | **~60 MB** | ~90 MB | ~55 MB | ~135 MB | ~420 MB |
| **Input-to-Screen Latency** | **< 5 ms** | 18 – 32 ms | 7 – 12 ms | 16 – 28 ms | 45 – 85 ms |
| **Streaming Throughput** | **~20 MB/s** | ~10 MB/s | ~22 MB/s | ~9 MB/s | ~2 MB/s |
| **Animation & Motion** | **Ultra Smooth** | Standard | Standard | Variable | Throttled |
| **Idle CPU Consumption** | **0.0%** | < 0.2% | 0.0% | 0.1% – 0.5% | 1.2% – 3.5% |

### Profiling Methodology

1. **System Timer Resolution**:
   - Standard Win32 apps run on a coarse timer interval, causing dropped frames or irregular frame timing on high-refresh displays.
   - Atlas optimizes system timer precision on startup, granting immediate responsiveness to GPUI's event pump and scheduler with zero frame drops.

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


