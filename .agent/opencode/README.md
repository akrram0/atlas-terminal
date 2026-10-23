# OpenCode "atlas-glass" theme

OpenCode's prompt input box is drawn as a **cell-aligned filled rectangle**
(`theme.decrease(theme.background.raised.base)`), with `╹`/`▀` half-block
hairlines gated on that colour's alpha. A terminal emulator cannot round an
arbitrary cell rectangle, so Atlas cannot fix those square corners from the
terminal side.

This theme makes the prompt fill fully transparent. OpenCode's own code then
skips the fill and the hairlines (`a !== 0` checks), leaving only the 1-cell
left accent rail, so there is no sharp rectangle to see over the acrylic.

## Install

The theme is installed for the current user at:

```
%USERPROFILE%\.config\opencode\themes\atlas-glass.json
```

and selected in `%USERPROFILE%\.config\opencode\tui.json`:

```json
{
  "theme": { "name": "atlas-glass" }
}
```

A backup of the previous `tui.json` is at `tui.json.bak-atlas-glass`.

OpenCode reads themes at startup; restart it (or use its theme picker) after
installing.

## How it was generated

Source: OpenCode **v2.0.32**, `packages/tui/src/theme/assets/v2/opencode.json`
(the built-in `opencode` theme, new hue-scaled format). Two edits:

1. `base.background.raised.base` -> `"transparent"` (removes the prompt fill).
2. `base["@dialog"].background.base` -> `"$hue.neutral.600"` (keeps dialogs
   opaque, since the built-in dialog inherited `background.raised.base`).

Everything else is unchanged.

## Revert

```powershell
Copy-Item "$env:USERPROFILE\.config\opencode\tui.json.bak-atlas-glass" `
          "$env:USERPROFILE\.config\opencode\tui.json" -Force
Remove-Item "$env:USERPROFILE\.config\opencode\themes\atlas-glass.json"
```

## Upstream

The real fix for *rounded* prompt corners belongs in OpenCode: its prompt
component already uses per-component `customBorderChars`, so a rounded border
option (or removing the bottom hairline) is a small upstream change. Atlas
should not attempt generic cell-rectangle rounding.
