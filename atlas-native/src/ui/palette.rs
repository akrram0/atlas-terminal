//! Command palette modal overlay and command catalog.

use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, AnyElement, ClickEvent, Context, ElementId, IntoElement,
    KeyDownEvent, Keystroke, Window, div, px,
};

use crate::icons;
use crate::session::Profile;
use crate::theme::Theme;
use crate::view::AtlasView;
use crate::workspace::SplitAxis;
use super::animation::{ease_apple_snappy, OVERLAY_IN_MS, OVERLAY_OUT_MS};

/// Commands offered by the palette.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Command {
    NewTab,
    NewTabWsl,
    SplitPane,
    SplitPaneDown,
    MaximizePane,
    ClosePane,
    CloseTab,
    NextTab,
    PrevTab,
    RestartTab,
    RenameTab,
    ToggleTheme,
    OpenSettings,
    FontInc,
    FontDec,
    CheckUpdates,
}

/// Palette entries: command, vector icon path, label, shortcut hint.
pub const COMMANDS: [(Command, &str, &str, &str); 16] = [
    (Command::NewTab, icons::PLUS, "New tab", "Ctrl Shift N"),
    (Command::NewTabWsl, icons::TERMINAL, "New Ubuntu (WSL) tab", ""),
    (Command::SplitPane, icons::SPLIT_COLUMNS, "Split pane right", "Ctrl Shift D"),
    (Command::SplitPaneDown, icons::SPLIT_ROWS, "Split pane down", "Ctrl Shift E"),
    (
        Command::MaximizePane,
        icons::WINDOW_MAXIMIZE,
        "Toggle maximize pane",
        "Ctrl Shift M",
    ),
    (Command::ClosePane, icons::CLOSE, "Close pane", "Ctrl Shift W"),
    (Command::CloseTab, icons::CLOSE, "Close tab", "Ctrl Shift W"),
    (Command::NextTab, icons::CHEVRON_RIGHT, "Next tab", "Ctrl Tab"),
    (
        Command::PrevTab,
        icons::CHEVRON_LEFT,
        "Previous tab",
        "Ctrl Shift Tab",
    ),
    (Command::RestartTab, icons::REFRESH, "Restart pane", ""),
    (Command::RenameTab, icons::PEN, "Rename tab", ""),
    (
        Command::ToggleTheme,
        icons::SUN,
        "Switch theme",
        "Ctrl Shift T",
    ),
    (Command::OpenSettings, icons::SETTINGS, "Open settings", ""),
    (Command::FontInc, icons::SETTINGS, "Increase font size", ""),
    (Command::FontDec, icons::SETTINGS, "Decrease font size", ""),
    (Command::CheckUpdates, icons::REFRESH, "Check for updates", ""),
];

impl AtlasView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = true;
        self.palette_closing = false;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_anim_index = 0.0;
        self.profile_menu = false;
        window.focus(&self.palette_focus);
        cx.notify();
    }

    /// Fade the palette out, then unmount it once the timer task fires; a
    /// fresh open cancels the pending dismissal.
    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_closing {
            return;
        }
        self.palette_closing = true;
        self.focus_main(window);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.palette_closing {
                    view.palette_closing = false;
                    view.palette_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Commands matching the current query, in declaration order.
    pub fn palette_matches(&self) -> Vec<(Command, &'static str, &'static str, &'static str)> {
        let query = self.palette_query.trim().to_ascii_lowercase();
        COMMANDS
            .iter()
            .copied()
            .filter(|(_, _, label, _)| {
                query.is_empty() || label.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// Palette key routing: type to filter, ↑/↓ to move, Enter to run, Esc to
    /// dismiss. Key events arrive here only while the palette holds focus, so
    /// nothing leaks into the shell.
    pub fn handle_palette_key(
        &mut self,
        ks: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ks.key.as_str() {
            "escape" => self.close_palette(window, cx),
            "enter" => {
                let matches = self.palette_matches();
                let index = self.palette_index.min(matches.len().saturating_sub(1));
                match matches.get(index).copied() {
                    Some((command, _, _, _)) => self.run_command(command, window, cx),
                    None => self.close_palette(window, cx),
                }
            }
            "up" => {
                self.palette_index = self.palette_index.saturating_sub(1);
                cx.notify();
            }
            "down" => {
                let last = self.palette_matches().len().saturating_sub(1);
                self.palette_index = (self.palette_index + 1).min(last);
                cx.notify();
            }
            "backspace" => {
                if self.palette_query.pop().is_some() {
                    self.palette_index = 0;
                }
                cx.notify();
            }
            _ => {
                if !ks.modifiers.control && !ks.modifiers.alt {
                    // The space bar arrives without `key_char` (gpui treats
                    // it as an immutable key), so type it by hand.
                    let typed: String = if ks.key == "space" {
                        " ".to_string()
                    } else {
                        ks.key_char
                            .as_deref()
                            .map(|text| {
                                text.chars().filter(|c| !c.is_control()).collect()
                            })
                            .unwrap_or_default()
                    };
                    if !typed.is_empty() {
                        self.palette_query.push_str(&typed);
                        self.palette_index = 0;
                        cx.notify();
                    }
                }
            }
        }
    }

    /// Run a palette command; the palette fades out on the way.
    pub fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_open {
            self.close_palette(window, cx);
        }
        match command {
            Command::NewTab => {
                self.new_tab(Profile::PowerShell, cx);
                window.focus(&self.focus);
            }
            Command::NewTabWsl => {
                self.new_tab(Profile::Wsl, cx);
                window.focus(&self.focus);
            }
            Command::SplitPane => {
                self.settings_open = false;
                self.split_active(SplitAxis::Columns, cx);
                window.focus(&self.focus);
            }
            Command::SplitPaneDown => {
                self.settings_open = false;
                self.split_active(SplitAxis::Rows, cx);
                window.focus(&self.focus);
            }
            Command::MaximizePane => {
                self.toggle_maximize(cx);
                window.focus(&self.focus);
            }
            Command::ClosePane => {
                self.close_pane(cx);
                self.focus_main(window);
            }
            Command::CloseTab => {
                let index = self.active;
                self.close_tab(index, cx);
                self.focus_main(window);
            }
            Command::NextTab => {
                self.cycle(1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::PrevTab => {
                self.cycle(-1);
                window.focus(&self.focus);
                cx.notify();
            }
            Command::RestartTab => {
                let index = self.active;
                self.restart_tab(index, cx);
                window.focus(&self.focus);
            }
            Command::RenameTab => self.open_rename(window, cx),
            Command::ToggleTheme => {
                self.toggle_theme(cx);
                self.focus_main(window);
            }
            Command::OpenSettings => {
                self.settings_open = true;
                self.profile_menu = false;
                window.focus(&self.settings_focus);
                cx.notify();
            }
            Command::FontInc => {
                self.font_size = (self.font_size + 1.0).min(24.0);
                self.focus_main(window);
                cx.notify();
            }
            Command::FontDec => {
                self.font_size = (self.font_size - 1.0).max(10.0);
                self.focus_main(window);
                cx.notify();
            }
            Command::CheckUpdates => {
                self.check_updates_manual(cx);
            }
        }
    }

    pub fn palette(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let matches = self.palette_matches();
        let closing = self.palette_closing;

        let mut text_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .flex_1()
            .text_size(px(13.0));
        if self.palette_query.is_empty() {
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(1.5)).h(px(17.0)).rounded_full().bg(theme.accent),
                );
            }
            text_row = text_row.child(div().text_color(theme.faint_fg).child("Type a command…"));
        } else {
            text_row = text_row.child(
                div()
                    .text_color(theme.chrome_fg_active)
                    .child(self.palette_query.clone()),
            );
            if self.cursor_on {
                text_row = text_row.child(
                    div().w(px(1.5)).h(px(17.0)).rounded_full().bg(theme.accent),
                );
            }
        }

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .h(px(46.0))
            .px(px(16.0))
            .text_color(theme.chrome_fg)
            .child(icons::icon(icons::SEARCH).size(px(14.0)).text_color(theme.chrome_fg))
            .child(text_row);

        let mut list = div().flex().flex_col().p(px(6.0)).relative();
        if matches.is_empty() {
            list = list.child(
                div()
                    .px(px(12.0))
                    .py(px(10.0))
                    .text_size(px(12.0))
                    .text_color(theme.faint_fg)
                    .child("No matching commands"),
            );
        } else {
            // Floating sliding selection pill (Raycast/Spotlight smooth glide)
            let pill_top = 6.0 + self.palette_anim_index * 36.0;
            list = list.child(
                div()
                    .absolute()
                    .left(px(6.0))
                    .right(px(6.0))
                    .top(px(pill_top))
                    .h(px(36.0))
                    .rounded(px(7.0))
                    .bg(theme.card_active),
            );

            for (index, (command, icon, label, shortcut)) in matches.into_iter().enumerate() {
                let hover = theme.card_hover;
                let row = div()
                    .id(ElementId::named_usize("palette-command", index))
                    .h(px(36.0))
                    .px(px(12.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(7.0))
                    .text_color(theme.chrome_fg)
                    .hover(move |style| style.bg(hover))
                    .child(icons::icon(icon).size(px(13.0)).text_color(theme.chrome_fg))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.5))
                            .text_color(theme.chrome_fg_active)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.faint_fg)
                            .child(shortcut),
                    )
                    .on_click(cx.listener(move |view, _: &ClickEvent, window, cx| {
                        view.run_command(command, window, cx);
                    }));
                list = list.child(row);
            }
        }

        div()
            .absolute()
            .top(px(54.0))
            .left(px(0.0))
            .right(px(0.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(
                div()
                    .id(ElementId::named_usize("palette-panel", 0))
                    .w(px(560.0))
                    .flex()
                    .flex_col()
                    .rounded(px(12.0))
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .track_focus(&self.palette_focus)
                    .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                        let keystroke = event.keystroke.clone();
                        view.handle_palette_key(&keystroke, window, cx);
                    }))
                    .child(input_row)
                    .child(div().h(px(1.0)).bg(theme.hairline))
                    .child(list),
            )
            .with_animation(
                ElementId::named_usize("palette-anim", closing as usize),
                Animation::new(Duration::from_millis(if closing {
                    OVERLAY_OUT_MS
                } else {
                    OVERLAY_IN_MS
                }))
                .with_easing(ease_apple_snappy),
                move |el, delta| {
                    let t = if closing { 1.0 - delta } else { delta };
                    el.opacity(t).top(px(54.0 - 8.0 * (1.0 - t)))
                },
            )
            .into_any_element()
    }
}
