---
name: Atlas Terminal Design System
description: Restrained, hardware-accelerated, dark-first terminal design system inspired by Zed, Zeron, and macOS craftsmanship.
colors:
  primary: "#c05a28"
  primary-hover: "#a84c1e"
  primary-dark: "#e08a62"
  primary-dark-hover: "#eba27e"
  bg-warm: "#ede8dc"
  bg-warm-elevated: "#f5f0e8"
  panel-warm: "#e5e0d4"
  fg-warm: "#1c1a16"
  muted-warm: "#7a7570"
  faint-warm: "#9a9590"
  bg-dark: "#0e0e0d"
  bg-dark-elevated: "#161614"
  panel-dark: "#141413"
  fg-dark: "#e8e4dc"
  muted-dark: "#8c8880"
  faint-dark: "#5a564f"
  border-warm: "rgba(0, 0, 0, 0.08)"
  border-dark: "rgba(255, 255, 255, 0.08)"
typography:
  display:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Inter', sans-serif"
    fontSize: "2.75rem"
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Inter', sans-serif"
    fontSize: "1.75rem"
    fontWeight: 600
    lineHeight: 1.25
    letterSpacing: "-0.02em"
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Inter', sans-serif"
    fontSize: "1.125rem"
    fontWeight: 600
    lineHeight: 1.4
    letterSpacing: "-0.01em"
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Inter', sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.6
    letterSpacing: "0em"
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Inter', sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: "0.02em"
  code:
    fontFamily: "'JetBrains Mono', 'SFMono-Regular', Consolas, monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0em"
rounded:
  sm: "6px"
  md: "10px"
  lg: "14px"
  full: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
  xxl: "48px"
  huge: "64px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.bg-warm-elevated}"
    rounded: "{rounded.sm}"
    padding: "10px 20px"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.bg-warm-elevated}"
    rounded: "{rounded.sm}"
    padding: "10px 20px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.fg-dark}"
    rounded: "{rounded.sm}"
    padding: "10px 20px"
  card:
    backgroundColor: "{colors.bg-dark-elevated}"
    textColor: "{colors.fg-dark}"
    rounded: "{rounded.md}"
    padding: "24px"
---

# Design System: Atlas Terminal

## Overview

**Creative North Star: "The Precision Instrument"**

Atlas Terminal embodies the restraint, mechanical precision, and tactile clarity of professional high-end hardware. Inspired by the engineering rigor of Zed and Zeron and the refined materials of macOS, the interface avoids decorative excess, loud gradients, and synthetic AI aesthetic clichés.

The design is dark-first, with a curated dual theme: a deep, non-pure black **Matte Dark** (`#0e0e0d` background, `#161614` elevated cards) and an archival **Paper Warm** (`#ede8dc` background, `#f5f0e8` elevated surfaces). Surface boundaries are articulated through razor-thin borders (`1px` with `8%` opacity) and subtle tonal steps rather than heavy drop shadows or colored halos.

**Key Characteristics:**
- **Zero Pure Black**: Deep zinc/obsidian tones (`#0e0e0d`, `#161614`) avoid harsh OLED clipping and eye strain while retaining high visual contrast.
- **Solid Typographic Hierarchy**: No decorative gradient text. Headings command attention through optical scale, balanced tracking, and weight.
- **Architectural Spacing**: Strict 8pt grid with 4pt half-steps, providing deliberate contrast between tight functional clusters and generous section breaks.
- **Authentic Engineering Artifacts**: Native vector SVG icons (Solar Linear 1.5px stroke), real monospace terminal data, and reproducible benchmark tables.

## Colors

The palette is anchored by warm terracotta and burnished copper accents set against deep neutral slate and parchment tones.

- **Dark Theme (Default UI)**:
  - Background Base: `#0e0e0d` (Matte Obsidian)
  - Panel / Chrome: `#141413`
  - Elevated / Card Surface: `#161614`
  - Primary Foreground: `#e8e4dc` (Warm Ivory)
  - Muted Foreground: `#8c8880` (Neutral Gray)
  - Faint / De-emphasized: `#5a564f`
  - Accent / Interactive: `#e08a62` (Warm Copper)
  - Accent Hover: `#eba27e`
  - Border Subtle: `rgba(255, 255, 255, 0.08)`
  - Border Active: `rgba(255, 255, 255, 0.22)`
- **Warm Theme (Paper Archive)**:
  - Background Base: `#faf7f0` / `#ede8dc`
  - Panel / Chrome: `#e5e0d4`
  - Elevated Surface: `#f5f0e8`
  - Primary Foreground: `#1c1a16`
  - Muted Foreground: `#7a7570`
  - Accent / Interactive: `#c05a28` (Terracotta)
  - Accent Hover: `#a84c1e`
  - Border Subtle: `rgba(0, 0, 0, 0.08)`

## Typography

Typography prioritizes scanability, mathematical alignment, and reading stamina.

- **System Font Cascade**: `-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Inter", sans-serif` ensures instantaneous zero-layout-shift rendering with native platform anti-aliasing.
- **Monospace Stack**: `"JetBrains Mono", "SFMono-Regular", Menlo, Monaco, Consolas, monospace` for CLI output, code snippets, keybindings, and tabular numerical data.
- **Scale Hierarchy**:
  - `Display / Hero`: `2.5rem` to `3rem`, weight `700`, line-height `1.15`, letter-spacing `-0.03em`, `text-wrap: balance`. Solid fill color (`var(--fg)`), never background-clip gradients.
  - `Section Headline (H2)`: `1.75rem` (`28px`), weight `600`, line-height `1.25`, letter-spacing `-0.02em`.
  - `Subsection Title (H3)`: `1.125rem` (`18px`), weight `600`, line-height `1.4`, letter-spacing `-0.01em`.
  - `Body Copy`: `1rem` (`16px`), weight `400`, line-height `1.6`, measure constrained to `60–72ch`, `text-wrap: pretty`.
  - `Meta / Footnotes / Badges`: Minimum `11px` (floor per WCAG and quality standards), weight `500`, letter-spacing `0.02em`.
- **Text Selection & Caret**: Themed from the palette (`::selection` uses `rgba(224, 138, 98, 0.25)` in dark and `rgba(192, 90, 40, 0.2)` in warm).

## Layout

Layout reinforces reading hierarchy through proximity rather than nesting.

- **Grid Alignment**: Built on a strict 8pt base grid (`4px`, `8px`, `16px`, `24px`, `32px`, `48px`, `64px`).
- **Container Widths**:
  - Main Page Container: `max-width: 1040px` with responsive horizontal padding (`20px` mobile, `32px` desktop).
  - Terminal Window Mockup: `max-width: 960px` centered with generous vertical margin.
- **No Eyebrows / Kickers**: Section tags like `[ 01 // ethos ]` and `[ 02 // capabilities ]` are removed in favor of strong, self-describing H2 titles.
- **Flat Section Topology**: Sections flow cleanly down the page with consistent top margins (`80px` desktop, `56px` mobile) and breathing room between functional groups.

## Elevation & Depth

Depth is tactile and physical, not decorative.

- **Surface Tiers**:
  - Level 0 (Base Canvas): `#0e0e0d`
  - Level 1 (Panel Chrome & Navigation): `#141413` with `backdrop-filter: blur(16px)`
  - Level 2 (Cards & Floating Windows): `#161614` with subtle `1px` border (`rgba(255, 255, 255, 0.08)`)
  - Level 3 (Menus & Modals): `#1c1c1a` with directional shadow: `0 16px 36px rgba(0, 0, 0, 0.45)`
- **No Zero-Offset Glowing Halos**: No chromatic halos (e.g. `0 0 8px rgba(47, 125, 50, 0.6)` or colored text shadows). Glow effects are replaced with clean, directional shadows and crisp border states.

## Shapes

- **Border Radius Tiers**:
  - `sm` (`6px`): Chips, badges, inputs, command key tags (`<kbd>`), and small buttons.
  - `md` (`10px`): Cards, interactive code blocks, and comparison table wrappers.
  - `lg` (`14px`): Terminal window mockup and modal frames.
  - `full` (`9999px`): Status pills, avatar badges, and theme toggle buttons.
- **Form Language**: Clean geometric rectangles with rounded corners, strictly preserving the 8pt rhythm.

## Components

- **Buttons**:
  - Primary: High-contrast solid accent (`#c05a28` / `#e08a62`) with ivory text, `6px` radius, subtle top-edge highlight, active scale down (`0.98`).
  - Secondary: Ghost / hairline bordered button (`1px solid var(--border)`), subtle background highlight on hover (`rgba(255, 255, 255, 0.05)`).
- **Cards**: Single-level containment. Never nest a card inside another card. Headers and bodies are separated by spacing, not divider lines or nested sub-boxes.
- **Terminal Mockup**: 1:1 reflection of Atlas Terminal GPUI window chrome — 36px topbar, process icons, subtle active tab pill, and monospace buffer.
- **Keycaps (`<kbd>`)**: Tactile hardware feel (`6px` radius, subtle inset highlight, `11px` monospace font).
- **Comparison Table**: Clean alternating rows with subtle border-bottom, highlighted column for Atlas Terminal with accent border tone.

## Do's and Don'ts

### Do
- Use solid `#e8e4dc` or `#1c1a16` for headings; emphasize with optical weight and scale.
- Maintain at least 4.5:1 text contrast for all functional and body copy.
- Group elements with proximity and whitespace before reaching for container boxes.
- Keep interactive functional text at or above `11px`.
- Use authentic drawn vector SVGs with consistent stroke weights.
- Ensure custom scrollbars, text selection, and focus rings match the design tokens.

### Don't
- **Don't** use gradient text (`background-clip: text`) on headings or stats.
- **Don't** use pure `#000000` for backgrounds.
- **Don't** use zero-offset neon glow shadows or chromatic halos.
- **Don't** nest cards inside cards.
- **Don't** use section number eyebrows (`[ 01 // ethos ]`) above headings.
- **Don't** use emoji or Unicode glyphs as functional interface icons.
- **Don't** allow functional interactive text to drop below `11px`.
