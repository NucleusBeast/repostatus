//! Restore the existing dashboard when the app is opened again. macOS sends
//! an Apple event; Windows launches a process that signals the first instance.

#[cfg(target_os = "macos")]
pub use native::{initialize, subscription};

#[cfg(target_os = "windows")]
pub use windows::{Instance, subscription};

#[cfg(not(target_os = "macos"))]
pub fn initialize() -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn subscription() -> iced::Subscription<()> {
    iced::Subscription::none()
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        io,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
        sync::Mutex,
        time::Duration,
    };
    use windows_sys::{
        Win32::{
            Foundation::{ERROR_ALREADY_EXISTS, GetLastError, WAIT_OBJECT_0},
            System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject},
            UI::WindowsAndMessaging::{
                AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId,
            },
        },
        core::w,
    };

    static EVENT: Mutex<Option<OwnedHandle>> = Mutex::new(None);

    pub struct Instance {
        _mutex: OwnedHandle,
    }

    impl Instance {
        pub fn acquire() -> io::Result<Option<Self>> {
            Self::acquire_named(
                w!("Local\\com.nucleusbeast.repostatus.instance"),
                w!("Local\\com.nucleusbeast.repostatus.reopen"),
            )
        }

        fn acquire_named(
            mutex_name: *const u16,
            event_name: *const u16,
        ) -> io::Result<Option<Self>> {
            // Create the auto-reset event first so even a launch during startup
            // can queue a reopen before Iced subscribes. Local scopes to this
            // Windows login session; the default ACL restricts access.
            // SAFETY: Names are NUL-terminated; null security uses OS defaults.
            let event = unsafe { CreateEventW(ptr::null(), 0, 0, event_name) };
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: Each successful create returns a newly owned handle.
            let event = unsafe { OwnedHandle::from_raw_handle(event) };
            let mutex = unsafe { CreateMutexW(ptr::null(), 0, mutex_name) };
            let error = unsafe { GetLastError() };
            if mutex.is_null() {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            let mutex = unsafe { OwnedHandle::from_raw_handle(mutex) };
            if error == ERROR_ALREADY_EXISTS {
                // Pass foreground permission from the user-initiated launch to
                // the existing process before it handles the reopen request.
                // SAFETY: The title is NUL-terminated and PID is a valid output.
                unsafe {
                    let window = FindWindowW(ptr::null(), w!("RepoStatus · Git Projects"));
                    if !window.is_null() {
                        let mut pid = 0;
                        GetWindowThreadProcessId(window, &mut pid);
                        if pid != 0 {
                            AllowSetForegroundWindow(pid);
                        }
                    }
                    if SetEvent(event.as_raw_handle()) == 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                return Ok(None);
            }
            *EVENT.lock().expect("reopen event mutex poisoned") = Some(event);
            Ok(Some(Self { _mutex: mutex }))
        }
    }

    impl Drop for Instance {
        fn drop(&mut self) {
            *EVENT.lock().expect("reopen event mutex poisoned") = None;
        }
    }

    pub fn subscription() -> iced::Subscription<()> {
        iced::time::every(Duration::from_millis(250)).filter_map(|_| take_request())
    }

    fn take_request() -> Option<()> {
        let event = EVENT.lock().expect("reopen event mutex poisoned");
        let event = event.as_ref()?;
        // SAFETY: The lock keeps the handle alive. A zero timeout never
        // blocks Iced; a successful wait consumes the auto-reset signal.
        (unsafe { WaitForSingleObject(event.as_raw_handle(), 0) } == WAIT_OBJECT_0).then_some(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn second_launch_queues_reopen_and_quit_allows_a_fresh_instance() {
            let name = format!("Local\\RepoStatus-test-{}", std::process::id());
            let mutex: Vec<u16> = format!("{name}-instance\0").encode_utf16().collect();
            let event: Vec<u16> = format!("{name}-reopen\0").encode_utf16().collect();
            let acquire = || Instance::acquire_named(mutex.as_ptr(), event.as_ptr()).unwrap();

            let first = acquire().expect("first launch must own the instance");
            assert_eq!(take_request(), None);
            // No subscription has started yet: startup must not lose requests.
            assert!(acquire().is_none());
            assert_eq!(take_request(), Some(()));
            assert_eq!(take_request(), None);
            assert!(acquire().is_none());
            assert_eq!(take_request(), Some(()));

            drop(first);
            let next = acquire().expect("quitting must release the instance");
            assert_eq!(take_request(), None);
            drop(next);
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use iced::{
        Subscription,
        futures::{Stream, channel::mpsc},
    };
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
    use objc2_foundation::{
        NSAppleEventDescriptor, NSAppleEventManager, NSObject, NSObjectProtocol,
    };
    use std::{cell::RefCell, sync::Mutex};

    static SENDER: Mutex<Option<mpsc::UnboundedSender<()>>> = Mutex::new(None);

    thread_local! {
        static HANDLER: RefCell<Option<Retained<ReopenHandler>>> = const { RefCell::new(None) };
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements. The handler is
        // created and called only on AppKit's main thread.
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[name = "RepoStatusReopenHandler"]
        struct ReopenHandler;

        unsafe impl NSObjectProtocol for ReopenHandler {}

        impl ReopenHandler {
            // SAFETY: This is NSAppleEventManager's documented handler signature.
            #[unsafe(method(handleReopen:withReplyEvent:))]
            fn handle_reopen(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
                if let Ok(sender) = SENDER.lock()
                    && let Some(sender) = sender.as_ref()
                {
                    let _ = sender.unbounded_send(());
                }
            }
        }
    );

    pub fn initialize() -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("Reopen handling requires the main thread.")?;
        HANDLER.with(|slot| {
            if slot.borrow().is_none() {
                // SAFETY: NSObject's initializer is valid for this subclass.
                let handler: Retained<ReopenHandler> =
                    unsafe { msg_send![ReopenHandler::alloc(mtm), init] };
                // SAFETY: The selector matches the method above. The handler
                // stays alive for the app's lifetime in the main-thread slot.
                unsafe {
                    NSAppleEventManager::sharedAppleEventManager()
                        .setEventHandler_andSelector_forEventClass_andEventID(
                            &handler,
                            sel!(handleReopen:withReplyEvent:),
                            u32::from_be_bytes(*b"aevt"), // kCoreEventClass
                            u32::from_be_bytes(*b"rapp"), // kAEReopenApplication
                        );
                }
                *slot.borrow_mut() = Some(handler);
            }
            Ok(())
        })
    }

    pub fn subscription() -> Subscription<()> {
        Subscription::run(events)
    }

    fn events() -> impl Stream<Item = ()> {
        let (sender, receiver) = mpsc::unbounded();
        *SENDER.lock().expect("reopen sender mutex poisoned") = Some(sender);
        receiver
    }
}
