use iced::{Task, window};

pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "windows"));

pub fn apply(id: window::Id, transparency: u8, keep_text_opaque: bool) -> Task<Result<(), String>> {
    window::run(id, move |window| {
        // Background-only transparency is rendered into the surface. Keep
        // native window alpha at 1 so it cannot fade the text a second time.
        set_transparency(window, native_transparency(transparency, keep_text_opaque))
    })
}

fn native_transparency(transparency: u8, keep_text_opaque: bool) -> u8 {
    if keep_text_opaque {
        0
    } else {
        transparency.min(50)
    }
}

pub fn background(color: iced::Color, transparency: u8, keep_text_opaque: bool) -> iced::Color {
    iced::Color {
        a: if SUPPORTED && keep_text_opaque {
            1.0 - f32::from(transparency.min(50)) / 100.0
        } else {
            1.0
        },
        ..color
    }
}

#[cfg(target_os = "macos")]
fn set_transparency(window: &dyn window::Window, transparency: u8) -> Result<(), String> {
    use iced::window::raw_window_handle::RawWindowHandle;
    use objc2_app_kit::NSView;

    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return Err("Could not access the macOS window.".into());
    };
    // Iced invokes window::run on the main event-loop thread. The borrowed
    // NSView pointer is valid for this callback and is never retained by us.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    let native_window = view.window().ok_or("Could not access the macOS window.")?;
    native_window.setAlphaValue(1.0 - f64::from(transparency) / 100.0);
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_transparency(window: &dyn window::Window, transparency: u8) -> Result<(), String> {
    use iced::window::raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError},
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongW,
            WS_EX_LAYERED,
        },
    };

    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("Could not access the Windows window.".into());
    };
    let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
    // The HWND remains valid during Iced's event-loop callback. Preserve all
    // other extended style flags, including the tray/window behavior.
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if transparency == 0 && style & WS_EX_LAYERED as i32 == 0 {
            return Ok(());
        }
        SetLastError(0);
        let new_style = if transparency == 0 {
            style & !(WS_EX_LAYERED as i32)
        } else {
            style | WS_EX_LAYERED as i32
        };
        let previous = SetWindowLongW(hwnd, GWL_EXSTYLE, new_style);
        if previous == 0 && GetLastError() != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if transparency == 0 {
            return Ok(());
        }
        let alpha = ((100 - u16::from(transparency)) * 255 / 100) as u8;
        if SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA) == 0 {
            let error = std::io::Error::last_os_error();
            SetWindowLongW(hwnd, GWL_EXSTYLE, style);
            return Err(error.to_string());
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_transparency(_: &dyn window::Window, _: u8) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Color;

    #[test]
    fn text_opacity_and_background_transparency_do_not_multiply() {
        assert_eq!(native_transparency(25, false), 25);
        assert_eq!(background(Color::BLACK, 25, false).a, 1.0);
        assert_eq!(native_transparency(25, true), 0);
        assert_eq!(
            background(Color::BLACK, 25, true).a,
            if SUPPORTED { 0.75 } else { 1.0 }
        );
        assert_eq!(background(Color::BLACK, 0, true).a, 1.0);
    }
}
