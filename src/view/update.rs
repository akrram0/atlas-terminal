//! Background update checks, GitHub release polling, delta downloading and restart management.

use std::time::Duration;

use gpui::*;

use crate::icons;
use crate::theme::Theme;
use crate::ui::animation::OVERLAY_OUT_MS;
use crate::updater;

use super::{AtlasView, UpdateDownloadState};

impl AtlasView {
    pub(crate) fn maybe_auto_check(&mut self, is_focus_event: bool, cx: &mut Context<Self>) {
        if !self.updater_settings.auto_check_updates || self.is_checking_updates {
            return;
        }

        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Rate limit pause: wait until GitHub quota resets
        if let Some(reset) = self.rate_limit_reset {
            if now_epoch < reset {
                return;
            }
        }

        if is_focus_event {
            // Window focus regain: only check if last check was > 1 hour ago
            if let Some(last) = self.updater_settings.last_check_time {
                if now_epoch.saturating_sub(last) < 3600 {
                    return;
                }
            }
        } else {
            // Background periodic check: check against configured interval or exponential backoff
            let interval_secs = (self.updater_settings.update_check_interval_mins as u64 * 60)
                .max(self.current_backoff_secs);
            if let Some(last) = self.updater_settings.last_check_time {
                if now_epoch.saturating_sub(last) < interval_secs {
                    return;
                }
            }
        }

        self.trigger_update_check(false, cx);
    }

    pub(crate) fn show_update_toast(&mut self, message: String, is_ok: bool, cx: &mut Context<Self>) {
        self.update_toast = Some((message, is_ok, std::time::Instant::now()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(3800)).await;
            let _ = this.update(cx, |view, cx| {
                if let Some((_, _, inst)) = view.update_toast {
                    if inst.elapsed() >= Duration::from_millis(3700) {
                        view.update_toast = None;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn render_update_toast(
        &self,
        message: &str,
        is_ok: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let text_color = theme.chrome_fg_active;
        let icon_path = if is_ok {
            if message.starts_with("Checking") {
                icons::REFRESH
            } else {
                icons::CHECK
            }
        } else {
            icons::CLOSE
        };
        let icon_color = if is_ok {
            theme.accent
        } else {
            rgb(0xE6A700)
        };

        let zoomed = crate::rounded_corners::is_zoomed();
        let toast_top = if zoomed { px(58.0) } else { px(50.0) };

        div()
            .id(ElementId::named_usize("update-toast-container", 0))
            .absolute()
            .top(toast_top)
            .left_0()
            .right_0()
            .flex()
            .flex_row()
            .justify_center()
            .items_center()
            .child(
                div()
                    .id(ElementId::named_usize("update-toast-pill", 0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .rounded_full()
                    .bg(theme.elevated_bg)
                    .border_1()
                    .border_color(theme.elevated_border)
                    .shadow_lg()
                    .cursor_pointer()
                    .on_click(cx.listener(|view, _: &ClickEvent, _window, cx| {
                        view.update_toast = None;
                        cx.notify();
                    }))
                    .child(
                        icons::icon(icon_path)
                            .size(px(14.0))
                            .text_color(icon_color),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_color)
                            .child(message.to_string()),
                    )
                    .child(
                        div()
                            .pl(px(4.0))
                            .text_color(theme.faint_fg)
                            .hover(|s| s.text_color(theme.fg))
                            .child(icons::icon(icons::CLOSE).size(px(11.0))),
                    ),
            )
            .into_any_element()
    }

    /// Manual update check triggered by button in top bar or Settings.
    pub(crate) fn check_updates_manual(&mut self, cx: &mut Context<Self>) {
        if self.is_checking_updates {
            return;
        }

        // 2 second cooldown on manual checks to prevent spam
        if let Some(last) = self.last_manual_check {
            if last.elapsed() < Duration::from_secs(2) {
                self.show_update_toast("Checking for updates...".to_string(), true, cx);
                return;
            }
        }

        self.last_manual_check = Some(std::time::Instant::now());
        self.show_update_toast("Checking for updates...".to_string(), true, cx);
        self.trigger_update_check(true, cx);
    }

    /// Spawn unblocked thread to run WinHTTP check against GitHub API.
    pub(crate) fn trigger_update_check(&mut self, is_manual: bool, cx: &mut Context<Self>) {
        if self.is_checking_updates {
            return;
        }
        self.is_checking_updates = true;
        if is_manual {
            self.manual_status_message = Some("Checking for updates...".to_string());
        }
        cx.notify();

        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let current_commit = option_env!("ATLAS_BUILD_COMMIT").unwrap_or("").to_string();
        let etag = if is_manual {
            None
        } else {
            self.updater_settings.etag.clone()
        };
        let channel = self.updater_settings.channel;

        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                updater::check_for_updates(&current_version, &current_commit, etag.as_deref(), channel)
            })
            .await;

            let _ = this.update(cx, |view, cx| {
                view.handle_update_result(result, is_manual, cx);
            });
        })
        .detach();
    }

    /// Process the result of an update check (auto or manual).
    pub(crate) fn handle_update_result(
        &mut self,
        result: updater::UpdateCheckResult,
        is_manual: bool,
        cx: &mut Context<Self>,
    ) {
        self.is_checking_updates = false;
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match result {
            updater::UpdateCheckResult::UpdateAvailable(info) => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = info.reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(ref e) = info.etag {
                    self.updater_settings.etag = Some(e.clone());
                }

                let is_skipped = self
                    .updater_settings
                    .skipped_version
                    .as_deref()
                    == Some(&info.version);

                if is_skipped {
                    if is_manual {
                        let msg = format!("Atlas v{} is available (skipped)", info.version);
                        self.manual_status_message = Some(msg.clone());
                        self.show_update_toast(msg, true, cx);
                    }
                } else {
                    self.available_update = Some(info.clone());
                    self.updater_settings.last_seen_version = Some(info.version.clone());
                    if is_manual {
                        self.manual_status_message =
                            Some(format!("v{} is available!", info.version));
                        self.update_toast = None;
                        self.update_panel_open = true;
                        self.update_panel_closing = false;
                    }
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::NotModified { etag, reset_epoch } => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(e) = etag {
                    self.updater_settings.etag = Some(e);
                }
                if is_manual {
                    let msg = format!("Atlas Terminal is up to date (v{})", env!("CARGO_PKG_VERSION"));
                    self.manual_status_message = Some(msg.clone());
                    self.show_update_toast(msg, true, cx);
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::UpToDate { etag, reset_epoch } => {
                self.current_backoff_secs = 3600;
                self.rate_limit_reset = reset_epoch;
                self.updater_settings.last_check_time = Some(now_epoch);
                if let Some(e) = etag {
                    self.updater_settings.etag = Some(e);
                }
                self.available_update = None;
                self.update_panel_open = false;
                self.update_panel_closing = false;
                self.update_download_state = UpdateDownloadState::Idle;
                if is_manual {
                    let msg = format!("Atlas Terminal is up to date (v{})", env!("CARGO_PKG_VERSION"));
                    self.manual_status_message = Some(msg.clone());
                    self.show_update_toast(msg, true, cx);
                }
                self.sync_and_save_settings();
            }
            updater::UpdateCheckResult::RateLimited { reset_epoch } => {
                self.rate_limit_reset = Some(reset_epoch);
                // Back off exponentially: 1h -> 2h -> 4h max
                self.current_backoff_secs = (self.current_backoff_secs * 2).min(14400);
                if is_manual {
                    let time_str = updater::format_epoch_time(reset_epoch);
                    let msg = format!("Rate limited, try again at {time_str}");
                    self.manual_status_message = Some(msg.clone());
                    self.show_update_toast(msg, false, cx);
                }
            }
            updater::UpdateCheckResult::Offline => {
                if is_manual {
                    let msg = "No internet connection".to_string();
                    self.manual_status_message = Some(msg.clone());
                    self.show_update_toast(msg, false, cx);
                }
            }
            updater::UpdateCheckResult::Error(msg) => {
                if is_manual {
                    let text = format!("Check failed: {msg}");
                    self.manual_status_message = Some(text.clone());
                    self.show_update_toast(text, false, cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn toggle_update_panel(&mut self, cx: &mut Context<Self>) {
        if self.update_panel_closing {
            self.update_panel_closing = false;
        } else {
            self.update_panel_open = !self.update_panel_open;
            if self.update_panel_open {
                self.palette_open = false;
                self.profile_menu = false;
            }
        }
        cx.notify();
    }

    pub(crate) fn dismiss_update_panel(&mut self, cx: &mut Context<Self>) {
        if self.update_panel_closing || !self.update_panel_open {
            return;
        }
        self.update_panel_closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OVERLAY_OUT_MS))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.update_panel_closing {
                    view.update_panel_closing = false;
                    view.update_panel_open = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn skip_current_update(&mut self, cx: &mut Context<Self>) {
        if let Some(update) = self.available_update.take() {
            self.updater_settings.skipped_version = Some(update.version);
            self.sync_and_save_settings();
        }
        self.dismiss_update_panel(cx);
    }

    pub(crate) fn start_download_update(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.available_update.clone() else {
            return;
        };

        if matches!(self.update_download_state, UpdateDownloadState::Downloading { .. }) {
            return;
        }

        let cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.update_cancel_token = Some(cancel_token.clone());
        self.update_download_state = UpdateDownloadState::Downloading {
            version: info.version.clone(),
            received_bytes: 0,
            total_bytes: info.download_size().unwrap_or(0),
            percent: 0.0,
        };
        cx.notify();

        let (progress_tx, progress_rx) = smol::channel::unbounded::<updater::DownloadProgress>();
        let (result_tx, result_rx) =
            smol::channel::bounded::<Result<updater::PreparedUpdate, String>>(1);
        let result_token = cancel_token.clone();
        let progress_token = cancel_token.clone();

        std::thread::spawn(move || {
            // Delta-first with automatic full-installer fallback.
            let res = updater::prepare_update(&info, cancel_token, move |prog| {
                let _ = progress_tx.send_blocking(prog);
            });
            let _ = result_tx.send_blocking(res);
        });

        cx.spawn(async move |this, cx| {
            while let Ok(prog) = progress_rx.recv().await {
                let _ = this.update(cx, |view, cx| {
                    let is_current = view
                        .update_cancel_token
                        .as_ref()
                        .is_some_and(|token| std::sync::Arc::ptr_eq(token, &progress_token));
                    if !is_current {
                        return;
                    }
                    if let UpdateDownloadState::Downloading { ref version, .. } = view.update_download_state {
                        let ver = version.clone();
                        view.update_download_state = UpdateDownloadState::Downloading {
                            version: ver,
                            received_bytes: prog.received_bytes,
                            total_bytes: prog.total_bytes,
                            percent: prog.percent,
                        };
                        cx.notify();
                    }
                });
            }

            if let Ok(res) = result_rx.recv().await {
                let _ = this.update(cx, |view, cx| {
                    // Ignore a result from a download the user cancelled or replaced.
                    let is_current = view
                        .update_cancel_token
                        .as_ref()
                        .is_some_and(|token| std::sync::Arc::ptr_eq(token, &result_token));
                    if !is_current {
                        return;
                    }
                    view.update_cancel_token = None;
                    let ver = view
                        .available_update
                        .as_ref()
                        .map(|u| u.version.clone())
                        .unwrap_or_default();
                    match res {
                        Ok(prepared) => {
                            view.update_download_state = UpdateDownloadState::ReadyToRestart {
                                version: ver,
                                prepared,
                            };
                        }
                        Err(err) => {
                            view.update_download_state = UpdateDownloadState::Failed {
                                version: ver,
                                error: err,
                            };
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn cancel_download_update(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.update_cancel_token.take() {
            token.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.update_download_state = UpdateDownloadState::Idle;
        cx.notify();
    }

    pub(crate) fn apply_update_and_restart(&mut self, cx: &mut Context<Self>) {
        let (prepared, ver) = if let UpdateDownloadState::ReadyToRestart { ref prepared, ref version } = self.update_download_state {
            (prepared.clone(), version.clone())
        } else {
            return;
        };

        self.updater_settings.etag = None;
        self.available_update = None;
        self.update_panel_open = false;
        self.sync_and_save_settings();

        if let Err(e) = updater::apply_update_and_restart(&prepared) {
            self.update_download_state = UpdateDownloadState::Failed {
                version: ver,
                error: e,
            };
            cx.notify();
        }
    }
}
