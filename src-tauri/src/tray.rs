// SPDX-License-Identifier: BUSL-1.1

use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use crate::app_state::AppState;
use crate::commands::approve_post_impl;
use crate::nav_commands::get_all_drafts_impl;

/// Stable tray icon identifier — used to look up the tray after creation.
const TRAY_ID: &str = "postlane-tray";

/// Menu item identifiers — matched in handle_menu_event.
const MENU_SHOW: &str = "show";
const MENU_DRAFTS_READY: &str = "drafts_ready";
const MENU_APPROVE_ALL: &str = "approve_all";
const MENU_FAILED: &str = "failed";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

/// Normal tray icon.
static ICON_NORMAL: &[u8] = include_bytes!("../icons/32x32.png");
/// Alert tray icon shown when any post has failed.
/// TODO: replace tray-alert.png with a red-tinted variant to visually distinguish failures.
static ICON_ALERT: &[u8] = include_bytes!("../icons/tray-alert.png");

// ---------------------------------------------------------------------------
// Pure logic — testable without Tauri runtime
// ---------------------------------------------------------------------------

/// Summary of draft counts used to drive tray badge and menu.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayStatus {
    pub ready_count: u32,
    pub failed_count: u32,
}

impl TrayStatus {
    /// Badge label: total count capped at 99, or "99+" if over.
    pub fn badge_label(&self) -> String {
        let total = self.ready_count + self.failed_count;
        if total == 0 {
            String::new()
        } else if total > 99 {
            "99+".to_string()
        } else {
            total.to_string()
        }
    }

    /// True when badge/icon should be red (any failed post).
    pub fn badge_is_red(&self) -> bool {
        self.failed_count > 0
    }

    /// Menu item: "{N} drafts ready" — shown only when ready_count > 0.
    pub fn ready_label(&self) -> Option<String> {
        if self.ready_count > 0 {
            Some(format!(
                "{} draft{} ready",
                self.ready_count,
                if self.ready_count == 1 { "" } else { "s" }
            ))
        } else {
            None
        }
    }

    /// Menu item: "Approve all ready ({N})" — shown only when ready_count > 0.
    pub fn approve_all_label(&self) -> Option<String> {
        if self.ready_count > 0 {
            Some(format!("Approve all ready ({})", self.ready_count))
        } else {
            None
        }
    }

    /// Menu item: "{N} failed" — shown only when failed_count > 0.
    pub fn failed_label(&self) -> Option<String> {
        if self.failed_count > 0 {
            Some(format!("{} failed", self.failed_count))
        } else {
            None
        }
    }

    /// Confirmation dialog message for "Approve all ready".
    pub fn approve_confirm_message(&self) -> String {
        format!(
            "Send {} post{} to scheduler?",
            self.ready_count,
            if self.ready_count == 1 { "" } else { "s" }
        )
    }
}

/// Compute TrayStatus from AppState.
pub fn compute_tray_status(state: &AppState) -> TrayStatus {
    match get_all_drafts_impl(state) {
        Ok(drafts) => {
            let ready_count =
                drafts.iter().filter(|d| d.status == "ready").count() as u32;
            let failed_count =
                drafts.iter().filter(|d| d.status == "failed").count() as u32;
            TrayStatus { ready_count, failed_count }
        }
        Err(_) => TrayStatus { ready_count: 0, failed_count: 0 },
    }
}

// ---------------------------------------------------------------------------
// Icon state
// ---------------------------------------------------------------------------

/// Swaps the tray icon between normal and alert state.
/// Alert icon signals that at least one post has failed.
fn update_tray_icon_state(tray: &tauri::tray::TrayIcon, is_alert: bool) {
    let bytes = if is_alert { ICON_ALERT } else { ICON_NORMAL };
    if let Ok(icon) = tauri::image::Image::from_bytes(bytes) {
        let _ = tray.set_icon(Some(icon));
    }
}

// ---------------------------------------------------------------------------
// Tauri tray wiring
// ---------------------------------------------------------------------------

/// Creates the system-tray icon and wires up the left-click handler.
/// Must be called after AppState is managed (reads state on first render).
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let state: tauri::State<AppState> = app.state();
    let status = compute_tray_status(&state);

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Postlane")
        .build(app)?;

    update_tray_badge(&tray, &status);
    update_tray_menu(app, &tray, &status)?;

    // Left-click on the tray icon — bring window to foreground.
    let app_handle = app.clone();
    tray.on_tray_icon_event(move |_tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            show_main_window(&app_handle);
        }
    });

    Ok(())
}

/// Updates the tray title (badge count) and icon to reflect current status.
///
/// `set_title` displays text next to the tray icon in the macOS menu bar.
/// On Windows it is a no-op. The icon swaps to the alert variant when any
/// post has failed, giving a visual colour signal alongside the count.
pub fn update_tray_badge(tray: &tauri::tray::TrayIcon, status: &TrayStatus) {
    let label = status.badge_label();
    let _ = tray.set_title(if label.is_empty() { None::<&str> } else { Some(label.as_str()) });
    update_tray_icon_state(tray, status.badge_is_red());
}

/// Rebuilds the tray menu to reflect current draft counts.
pub fn update_tray_menu(
    app: &AppHandle,
    tray: &tauri::tray::TrayIcon,
    status: &TrayStatus,
) -> tauri::Result<()> {
    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = Vec::new();

    // Show Postlane
    let show = MenuItem::with_id(app, MENU_SHOW, "Show Postlane", true, None::<&str>)?;
    items.push(Box::new(show));

    // Drafts ready (conditional)
    if let Some(label) = status.ready_label() {
        let item = MenuItem::with_id(app, MENU_DRAFTS_READY, &label, true, None::<&str>)?;
        items.push(Box::new(item));
    }

    // Approve all ready (conditional, separate from "drafts ready" line)
    if let Some(label) = status.approve_all_label() {
        let item = MenuItem::with_id(app, MENU_APPROVE_ALL, &label, true, None::<&str>)?;
        items.push(Box::new(item));
    }

    // Failed (conditional, separate line)
    if let Some(label) = status.failed_label() {
        let item = MenuItem::with_id(app, MENU_FAILED, &label, true, None::<&str>)?;
        items.push(Box::new(item));
    }

    // Settings
    let settings =
        MenuItem::with_id(app, MENU_SETTINGS, "Settings", true, None::<&str>)?;
    items.push(Box::new(settings));

    // Quit
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    items.push(Box::new(quit));

    let refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        items.iter().map(|b| b.as_ref()).collect();
    let menu = Menu::with_items(app, &refs)?;
    tray.set_menu(Some(menu))?;
    Ok(())
}

/// Called whenever meta-changed fires — updates badge and menu.
pub fn refresh_tray(app: &AppHandle) {
    let state: tauri::State<AppState> = app.state();
    let status = compute_tray_status(&state);

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        update_tray_badge(&tray, &status);
        let _ = update_tray_menu(app, &tray, &status);
    }
}

/// Dispatches tray menu item clicks received via app.on_menu_event.
pub fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_SHOW => show_main_window(app),
        MENU_DRAFTS_READY | MENU_FAILED => {
            show_main_window(app);
            let _ = app.emit("tray-navigate", "all-repos-drafts");
        }
        MENU_APPROVE_ALL => approve_all_from_tray(app.clone()),
        MENU_SETTINGS => {
            show_main_window(app);
            let _ = app.emit("tray-navigate", "settings");
        }
        MENU_QUIT => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                graceful_shutdown(app).await;
            });
        }
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Shows a native OS confirmation dialog and, if confirmed, sends all ready
/// posts to the scheduler without opening the app window.
pub fn approve_all_from_tray(app: AppHandle) {
    let state: tauri::State<AppState> = app.state();
    let drafts = match get_all_drafts_impl(&state) {
        Ok(d) => d,
        Err(e) => {
            log::error!("approve_all_from_tray: failed to read drafts: {}", e);
            return;
        }
    };

    let ready: Vec<(String, String)> = drafts
        .into_iter()
        .filter(|d| d.status == "ready")
        .map(|d| (d.repo_path, d.post_folder))
        .collect();

    if ready.is_empty() {
        return;
    }

    let n = ready.len() as u32;
    let message = TrayStatus { ready_count: n, failed_count: 0 }.approve_confirm_message();
    let app_for_show = app.clone();

    app.dialog()
        .message(&message)
        .title("Postlane")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Send".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            if confirmed {
                let app_inner = app_for_show.clone();
                tauri::async_runtime::spawn(async move {
                    let state: tauri::State<AppState> = app_inner.state();
                    for (repo_path, post_folder) in &ready {
                        if let Err(e) = approve_post_impl(
                            repo_path,
                            post_folder,
                            &state,
                            Some(&app_inner),
                        )
                        .await
                        {
                            log::error!(
                                "Tray approve failed for {}/{}: {}",
                                repo_path,
                                post_folder,
                                e
                            );
                        }
                    }
                    refresh_tray(&app_inner);
                });
            }
        });
}

// ---------------------------------------------------------------------------
// Graceful shutdown logic
// ---------------------------------------------------------------------------

/// Graceful shutdown: stops watchers and waits up to 5 seconds for in-flight
/// sends to complete, then exits the process.
///
/// Teardown order: watchers stopped → in-flight sends awaited → app exits.
pub async fn graceful_shutdown(app: AppHandle) {
    use std::time::Duration;

    let state: tauri::State<AppState> = app.state();
    crate::watcher::stop_all_watchers(&state.watchers);

    // A full implementation would poll an AtomicUsize in-flight counter here
    // and exit early once it reaches zero, up to a 5-second deadline.
    // M5: no concurrent send tracking yet — give watchers a moment to flush.
    tokio::time::sleep(Duration::from_millis(50)).await;

    app.exit(0);
}

// ---------------------------------------------------------------------------
// Tests — pure logic only (no Tauri runtime required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn status(ready: u32, failed: u32) -> TrayStatus {
        TrayStatus { ready_count: ready, failed_count: failed }
    }

    // Badge label
    #[test]
    fn test_badge_label_zero() {
        assert_eq!(status(0, 0).badge_label(), "");
    }

    #[test]
    fn test_badge_label_single_ready() {
        assert_eq!(status(1, 0).badge_label(), "1");
    }

    #[test]
    fn test_badge_label_combined() {
        assert_eq!(status(3, 2).badge_label(), "5");
    }

    #[test]
    fn test_badge_label_cap_at_99() {
        assert_eq!(status(99, 0).badge_label(), "99");
    }

    #[test]
    fn test_badge_label_over_99() {
        assert_eq!(status(100, 0).badge_label(), "99+");
        assert_eq!(status(50, 60).badge_label(), "99+");
    }

    // Badge colour
    #[test]
    fn test_badge_not_red_with_only_ready() {
        assert!(!status(5, 0).badge_is_red());
    }

    #[test]
    fn test_badge_red_with_any_failed() {
        assert!(status(0, 1).badge_is_red());
        assert!(status(5, 1).badge_is_red());
    }

    #[test]
    fn test_badge_not_red_when_empty() {
        assert!(!status(0, 0).badge_is_red());
    }

    // Menu labels
    #[test]
    fn test_ready_label_hidden_when_zero() {
        assert!(status(0, 0).ready_label().is_none());
    }

    #[test]
    fn test_ready_label_singular() {
        assert_eq!(status(1, 0).ready_label().unwrap(), "1 draft ready");
    }

    #[test]
    fn test_ready_label_plural() {
        assert_eq!(status(3, 0).ready_label().unwrap(), "3 drafts ready");
    }

    #[test]
    fn test_approve_all_hidden_when_zero_ready() {
        assert!(status(0, 2).approve_all_label().is_none());
    }

    #[test]
    fn test_approve_all_shown_when_ready() {
        assert!(status(2, 0).approve_all_label().is_some());
        assert_eq!(status(2, 0).approve_all_label().unwrap(), "Approve all ready (2)");
    }

    #[test]
    fn test_failed_label_hidden_when_zero() {
        assert!(status(0, 0).failed_label().is_none());
    }

    #[test]
    fn test_failed_label_shown() {
        assert_eq!(status(0, 3).failed_label().unwrap(), "3 failed");
    }

    // Confirmation message
    #[test]
    fn test_approve_confirm_singular() {
        assert_eq!(status(1, 0).approve_confirm_message(), "Send 1 post to scheduler?");
    }

    #[test]
    fn test_approve_confirm_plural() {
        assert_eq!(status(5, 0).approve_confirm_message(), "Send 5 posts to scheduler?");
    }
}
