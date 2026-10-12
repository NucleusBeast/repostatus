use iced::Subscription;

pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Toggle,
    Refresh,
    Quit,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use native::{initialize, subscription};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn initialize() -> Result<(), String> {
    Err("Background tray operation is supported on macOS and Windows.".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn subscription() -> Subscription<Command> {
    Subscription::none()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod native {
    use super::{Command, Subscription};
    use iced::futures::{Stream, channel::mpsc};
    use std::cell::RefCell;
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    // Native tray objects must stay on the window event-loop thread. Initialization
    // runs through window::run, after Iced has opened its first window.
    thread_local! {
        static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
    }

    pub fn initialize() -> Result<(), String> {
        TRAY.with(|slot| {
            if slot.borrow().is_some() {
                return Ok(());
            }
            let create = || -> Result<TrayIcon, Box<dyn std::error::Error>> {
                let menu = Menu::new();
                menu.append_items(&[
                    &MenuItem::with_id("toggle", "Toggle RepoStatus", true, None),
                    &MenuItem::with_id("refresh", "Refresh repositories", true, None),
                    &PredefinedMenuItem::separator(),
                    &MenuItem::with_id("quit", "Quit RepoStatus", true, None),
                ])?;
                Ok(TrayIconBuilder::new()
                    .with_tooltip("RepoStatus · Git Projects")
                    .with_icon(icon()?)
                    .with_icon_as_template(cfg!(target_os = "macos"))
                    .with_menu(Box::new(menu))
                    .with_menu_on_left_click(false)
                    .build()?)
            };
            let tray = create().map_err(|error| {
                format!("Could not create the tray icon: {error}. Closing the window will quit RepoStatus.")
            })?;
            *slot.borrow_mut() = Some(tray);
            Ok(())
        })
    }

    pub fn subscription() -> Subscription<Command> {
        Subscription::run(events)
    }

    fn events() -> impl Stream<Item = Command> {
        let (sender, receiver) = mpsc::unbounded();
        let menu_sender = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = match event.id.as_ref() {
                "toggle" => Command::Toggle,
                "refresh" => Command::Refresh,
                "quit" => Command::Quit,
                _ => return,
            };
            let _ = menu_sender.unbounded_send(command);
        }));
        TrayIconEvent::set_event_handler(Some(move |event| {
            // Left-click toggles the dashboard on both platforms.
            // The native tray menu remains available on right-click.
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = sender.unbounded_send(Command::Toggle);
            }
        }));
        receiver
    }

    fn icon() -> Result<Icon, tray_icon::BadIcon> {
        // A crisp R glyph at tray size. macOS uses its alpha as a template so
        // the system adapts it to light/dark menu bars; Windows gets a red tile.
        let glyph = [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ];
        let mut rgba = vec![0; 32 * 32 * 4];
        for y in 0..32usize {
            for x in 0..32usize {
                let pixel = &mut rgba[(y * 32 + x) * 4..][..4];
                if cfg!(target_os = "windows") {
                    let dx = 5usize.saturating_sub(x).max(x.saturating_sub(26));
                    let dy = 5usize.saturating_sub(y).max(y.saturating_sub(26));
                    if dx * dx + dy * dy <= 25 {
                        pixel.copy_from_slice(&[220, 38, 38, 255]);
                    }
                }
                if (8..23).contains(&x)
                    && (5..26).contains(&y)
                    && glyph[(y - 5) / 3].as_bytes()[(x - 8) / 3] == b'1'
                {
                    pixel.copy_from_slice(if cfg!(target_os = "macos") {
                        &[0, 0, 0, 255]
                    } else {
                        &[255, 255, 255, 255]
                    });
                }
            }
        }
        Icon::from_rgba(rgba, 32, 32)
    }
}
