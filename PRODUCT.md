# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Windows developers, DevOps/infrastructure engineers, systems programmers, and terminal power users who spend hours daily inside shells (PowerShell, WSL, cmd, ssh) and want a high-performance terminal with instantaneous launch times, high refresh rates, and a clean, distraction-free environment.

## Product Purpose

Atlas Terminal provides a high-velocity, GPU-accelerated terminal emulator for Windows engineered in Rust on Zed's GPUI framework. It exists to deliver the raw hardware throughput, typography precision, and aesthetic restraint of modern macOS tools without the memory bloat of Electron or the baseline rendering defects of legacy consoles.

## Positioning

Unlike Windows Terminal, Alacritty, or WezTerm, Atlas Terminal unites DirectX 12 hardware rasterization with ultra-smooth native frame scheduling, zero-overhead native DWM Acrylic blur, procedural MSAA vector prompt arcs (no font baseline clipping or disconnected box-drawing glyphs), and native iTerm2 inline graphics (OSC 1337) with a lightweight working memory footprint of ~60MB.

## Operating Context

- **Operating System**: Windows 11 and Windows 10 (x64 native binary, packaged via standalone installer and `winget`).
- **Primary Shell Environments**: PowerShell 7+, Windows Subsystem for Linux (WSL2: Ubuntu, Arch, Debian, Fedora, Alpine), Command Prompt, and developer tooling (fastfetch, git, neovim, fzf, Starship).
- **Surface**: High-conversion open-source marketing landing page, release distribution hub, and documentation center.

## Capabilities and Constraints

- **Rendering Engine**: Rust GPUI with DirectX 12 backend.
- **Refresh & Timing**: Adaptive native display frame scheduling with zero frame drops, dropping to 0% CPU consumption when idle.
- **Inline Graphics**: iTerm2 OSC 1337 protocol natively integrated into text layout.
- **Aesthetic**: Restrained Apple/macOS design language, DWM Acrylic translucency, no decorative AI gradients, no glowing halos, no nested cards, no pure `#000000`.
- **Distribution**: Standalone signed installer, automatic cryptographic delta updater with SHA-256 verification, and Microsoft Windows Package Manager (`winget`).

## Brand Commitments

- **Tone**: Technical authority, empirical honesty, engineering precision, quiet confidence.
- **Identity**: Solar/Linear vector icons, disciplined typography (system SF Pro / Inter font stack paired with JetBrains Mono), intentional whitespace.
- **Visual Anti-references**: Neon cyber-glows, glassmorphic blur without purpose, generic marketing gradient text, stacked metric cards, and decorative AI slop.

## Evidence on Hand

- Empirical benchmark suites comparing cold startup, working set memory, input latency, and streaming throughput against Windows Terminal, Alacritty, WezTerm, and Hyper (`scripts/benchmark.ps1`).
- Real interactive in-browser terminal sandbox running simulated shells, themes, and fastfetch output.
- Signed GitHub releases with SHA-256 binary manifests.

## Product Principles

1. **Empirical Precision Over Fluff**: State exact benchmark numbers, milliseconds, and megabytes. Let the engineering speak without exaggerated marketing superlatives.
2. **Restraint and Focus**: Tools should fade into the background. Every pixel, border, and color must justify its existence.
3. **Respect Developer Conventions**: Home-row ergonomics, standard keybindings, copy-paste terminal workflows, and verifiable open-source artifacts.
4. **Performance is a Feature**: Fast loading, 0% idle CPU, zero runtime dependencies, and instant interactive feedback.

## Accessibility & Inclusion

- Adherence to WCAG 2.1 AA standards: minimum 4.5:1 text contrast for body copy and 3:1 for large text across all light/dark theme tokens.
- Explicit keyboard focus rings and full keyboard navigation for interactive widgets and command prompts.
- Respect for user motion preferences (`prefers-reduced-motion`) and system theme preferences (`prefers-color-scheme`).
