//! Per-user launch-at-login registration. The OS entry is the source of truth.

pub fn is_enabled() -> Result<bool, String> {
    platform::is_enabled()
        .map_err(|error| format!("Could not read launch-at-login setting: {error}"))
}

pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    platform::set_enabled(enabled)
        .map(|()| enabled)
        .map_err(|error| format!("Could not change launch-at-login setting: {error}"))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod platform {
    use std::{
        fs, io,
        path::{Path, PathBuf},
    };

    fn entry_path() -> io::Result<PathBuf> {
        #[cfg(target_os = "macos")]
        let path = dirs::home_dir()
            .map(|home| home.join("Library/LaunchAgents/com.nucleusbeast.repostatus.plist"));
        #[cfg(target_os = "linux")]
        let path = dirs::config_dir().map(|config| config.join("autostart/repostatus.desktop"));
        path.ok_or_else(|| io::Error::other("Could not locate the user startup directory."))
    }

    pub fn is_enabled() -> io::Result<bool> {
        entry_path()?.try_exists()
    }

    pub fn set_enabled(enabled: bool) -> io::Result<()> {
        let path = entry_path()?;
        let entry = if enabled {
            Some(entry_contents(&std::env::current_exe()?)?)
        } else {
            None
        };
        write_entry(&path, entry.as_deref())
    }

    fn write_entry(path: &Path, contents: Option<&str>) -> io::Result<()> {
        if let Some(contents) = contents {
            fs::create_dir_all(
                path.parent()
                    .ok_or_else(|| io::Error::other("Invalid startup path"))?,
            )?;
            let temporary = path.with_extension("tmp");
            fs::write(&temporary, contents)?;
            fs::rename(temporary, path)
        } else {
            match fs::remove_file(path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }

    fn entry_contents(executable: &Path) -> io::Result<String> {
        let executable = executable.to_str().ok_or_else(|| {
            io::Error::other("The application path must be valid UTF-8 to start at login.")
        })?;
        if executable.chars().any(char::is_control) {
            return Err(io::Error::other(
                "The application path contains control characters.",
            ));
        }
        #[cfg(target_os = "macos")]
        {
            let executable = executable
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;");
            Ok(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.nucleusbeast.repostatus</string>
<key>ProgramArguments</key><array><string>{executable}</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>
"#
            ))
        }
        #[cfg(target_os = "linux")]
        {
            // Exec quoting is followed by desktop-entry string escaping.
            let mut quoted = String::new();
            for character in executable.chars() {
                if matches!(character, '"' | '`' | '$' | '\\') {
                    quoted.push('\\');
                }
                quoted.push(character);
                if character == '%' {
                    quoted.push('%');
                }
            }
            let quoted = quoted.replace('\\', "\\\\");
            Ok(format!(
                "[Desktop Entry]\nType=Application\nName=RepoStatus\nExec=\"{quoted}\"\nTerminal=false\n"
            ))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn registration_can_be_enabled_updated_and_disabled() {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("startup/entry");
            write_entry(&path, Some("first executable")).unwrap();
            write_entry(&path, Some("moved executable")).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), "moved executable");
            write_entry(&path, None).unwrap();
            write_entry(&path, None).unwrap();
            assert!(!path.exists());
        }

        #[test]
        fn executable_paths_are_escaped() {
            let entry =
                entry_contents(Path::new("/Apps/Repo & \"$`%\\Status.app/repostatus")).unwrap();
            #[cfg(target_os = "macos")]
            assert!(entry.contains("/Apps/Repo &amp; &quot;$`%\\Status.app/repostatus"));
            #[cfg(target_os = "linux")]
            assert!(entry.contains(r#"Exec="/Apps/Repo & \\"\\$\\`%%\\\\Status.app/repostatus""#));
            assert!(entry_contents(Path::new("/Apps/line\nbreak")).is_err());
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::*,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    fn check(result: u32) -> io::Result<()> {
        if result == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(result as i32))
        }
    }

    pub fn is_enabled() -> io::Result<bool> {
        let key = wide(OsStr::new(RUN_KEY));
        let name = wide(OsStr::new("RepoStatus"));
        // No data buffer is needed to check whether this named value exists.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        check(result).map(|()| true)
    }

    pub fn set_enabled(enabled: bool) -> io::Result<()> {
        let key = wide(OsStr::new(RUN_KEY));
        let name = wide(OsStr::new("RepoStatus"));
        if !enabled {
            let result =
                unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
            return if result == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                check(result)
            };
        }
        let executable = std::env::current_exe()?;
        // current_exe can return a verbatim path, which Windows Run does not need.
        let mut command: Vec<u16> = executable.as_os_str().encode_wide().collect();
        let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
        if command.starts_with(&prefix) {
            command.drain(..prefix.len());
            let unc: Vec<u16> = "UNC\\".encode_utf16().collect();
            if command.starts_with(&unc) {
                command.splice(..unc.len(), "\\\\".encode_utf16());
            }
        }
        command.insert(0, '"' as u16);
        command.extend(['"' as u16, 0]);
        if command.len() > 260 {
            return Err(io::Error::other(
                "The application path is too long for Windows startup (260 characters).",
            ));
        }
        let mut handle = ptr::null_mut();
        unsafe {
            check(RegCreateKeyW(HKEY_CURRENT_USER, key.as_ptr(), &mut handle))?;
            let result = RegSetValueExW(
                handle,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            );
            RegCloseKey(handle);
            check(result)
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod platform {
    pub fn is_enabled() -> Result<bool, &'static str> {
        Err("Unsupported platform.")
    }
    pub fn set_enabled(_: bool) -> Result<(), &'static str> {
        Err("Unsupported platform.")
    }
}
