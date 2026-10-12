use iced::{Task, window};

pub const DOCK_ERROR: &str = "macOS could not change RepoStatus's Dock visibility.";

pub fn toggle(id: window::Id) -> Task<Result<(), String>> {
    // Query the native state so closing/minimizing the window cannot leave
    // the tray toggle out of sync with what the user actually sees.
    window::mode(id).then(move |mode| {
        if mode == window::Mode::Hidden {
            show(id)
        } else {
            window::is_minimized(id).then(move |minimized| {
                if minimized == Some(true) {
                    show(id)
                } else {
                    hide(id)
                }
            })
        }
    })
}

pub fn hide(id: window::Id) -> Task<Result<(), String>> {
    // Hide the window before switching to an accessory app, so macOS does
    // not leave a visible window behind when removing the Dock icon.
    window::set_mode(id, window::Mode::Hidden).chain(window::run(id, |_| set_dock_visible(false)))
}

pub fn show(id: window::Id) -> Task<Result<(), String>> {
    // Changing activation policy can reorder windows. Restore regular app
    // status first, then show and focus the dashboard.
    window::run(id, |_| set_dock_visible(true)).then(move |result| {
        window::set_mode(id, window::Mode::Windowed)
            .chain(window::minimize(id, false))
            .chain(window::gain_focus(id))
            .chain(Task::done(result))
    })
}

#[cfg(target_os = "macos")]
fn set_dock_visible(visible: bool) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    // window::run executes this callback on Iced's native event-loop thread.
    let main_thread = MainThreadMarker::new().ok_or("Dock changes require the main thread.")?;
    let app = NSApplication::sharedApplication(main_thread);
    let policy = if visible {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    // AppKit returns false when asked to set a policy that is already active.
    // Repeated tray clicks and close requests must still count as success.
    if app.activationPolicy() == policy
        || app.setActivationPolicy(policy)
        || app.activationPolicy() == policy
    {
        Ok(())
    } else {
        Err(DOCK_ERROR.into())
    }
}

#[cfg(not(target_os = "macos"))]
fn set_dock_visible(_: bool) -> Result<(), String> {
    Ok(())
}
