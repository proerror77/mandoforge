use crate::DesktopState;
use std::io::Write;
use std::process::{Command, Stdio};
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_console = MenuItem::with_id(app, "show_console", "Show Console", true, None::<&str>)?;
    let open_browser = MenuItem::with_id(app, "open_browser", "Open Browser", true, None::<&str>)?;
    let copy_api_url = MenuItem::with_id(app, "copy_api_url", "Copy API URL", true, None::<&str>)?;
    let open_logs = MenuItem::with_id(app, "open_logs", "Open Logs", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "status", "Status", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_console,
            &open_browser,
            &copy_api_url,
            &open_logs,
            &status,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("mandoforge")
        .tooltip("MandoForge Control Plane")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_console" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "open_browser" => {
                let state = app.state::<DesktopState>();
                let _ = opener::open(state.api_base_url());
            }
            "copy_api_url" => {
                let state = app.state::<DesktopState>();
                copy_to_clipboard(state.api_base_url()).ok();
            }
            "open_logs" => {
                if let Ok(path) = crate::logs_dir() {
                    let _ = opener::open(path);
                }
            }
            "status" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_to_clipboard(value: &str) -> std::io::Result<()> {
    write_to_clipboard_command("pbcopy", &[], value)
}

#[cfg(target_os = "windows")]
fn copy_to_clipboard(value: &str) -> std::io::Result<()> {
    write_to_clipboard_command("clip", &[], value)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn copy_to_clipboard(value: &str) -> std::io::Result<()> {
    write_to_clipboard_command("wl-copy", &[], value)
        .or_else(|_| write_to_clipboard_command("xclip", &["-selection", "clipboard"], value))
}

fn write_to_clipboard_command(command: &str, args: &[&str], value: &str) -> std::io::Result<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(value.as_bytes())?;
    }
    let _ = child.wait()?;
    Ok(())
}
