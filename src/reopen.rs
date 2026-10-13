//! Opening a running macOS app sends a reopen Apple event rather than
//! starting another process. Forward it to Iced without replacing Winit's
//! application delegate, which owns the event loop.

#[cfg(target_os = "macos")]
pub use native::{initialize, subscription};

#[cfg(not(target_os = "macos"))]
pub fn initialize() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn subscription() -> iced::Subscription<()> {
    iced::Subscription::none()
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
