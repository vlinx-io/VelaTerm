//! Compatibility with WebKit versions that forward mouse tracking events to covered WKWebViews.
//!
//! The host and browser are sibling native views. Older WKMouseTrackingObserver implementations also
//! forward movement to the covered host, whose default cursor can overwrite the browser's link cursor.
//! Mirror WebKit's topmost-view check (312201@main) for the host's tracking observer only. Keep its
//! tracking areas and original implementations intact; do not modify global WebKit classes or pages.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

use objc2::rc::Weak;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::NSPoint;

struct TrackingState {
    view: Weak<AnyObject>,
    was_topmost: bool,
}

thread_local! {
    // Accessed only on AppKit's main thread. Weak references do not keep closed windows alive.
    static HOSTS: RefCell<HashMap<usize, TrackingState>> = RefCell::new(HashMap::new());
}

fn observer_class(base: &'static AnyClass) -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"VelaBrowserHostMouseTracking", base)
            .expect("unique browser mouse-tracking class");
        // No ivars are added: existing observer instances retain their original size and layout.
        // Use C-unwind so Objective-C exceptions raised beneath AppKit/WebKit mouse tracking can unwind
        // back to NSApplication instead of hitting a Rust nounwind abort.
        unsafe {
            builder.add_method(sel!(mouseMoved:), mouse_moved as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(mouseEntered:), mouse_entered as extern "C-unwind" fn(_, _, _));
            builder.add_method(sel!(mouseExited:), mouse_exited as extern "C-unwind" fn(_, _, _));
        }
        builder.register()
    })
}

/// AppKit hitTest accepts a point in the receiver's superview coordinate system, not its own.
unsafe fn is_topmost(view: &AnyObject, event: &AnyObject) -> bool {
    let window: *mut AnyObject = msg_send![view, window];
    if window.is_null() {
        return false;
    }
    let content: *mut AnyObject = msg_send![window, contentView];
    if content.is_null() {
        return false;
    }
    let parent: *mut AnyObject = msg_send![content, superview];
    let location: NSPoint = msg_send![event, locationInWindow];
    let point = if parent.is_null() {
        location
    } else {
        msg_send![parent, convertPoint: location, fromView: std::ptr::null::<AnyObject>()]
    };
    let hit: *mut AnyObject = msg_send![content, hitTest: point];
    !hit.is_null() && msg_send![hit, isDescendantOf: view]
}

fn should_forward(observer: &AnyObject, event: &AnyObject, exiting: bool) -> bool {
    HOSTS.with(|hosts| {
        let mut hosts = hosts.borrow_mut();
        let Some(state) = hosts.get_mut(&(observer as *const AnyObject as usize)) else {
            return true;
        };
        if exiting {
            // Exit coordinates are outside the view; use the preceding move/enter hit test.
            return std::mem::replace(&mut state.was_topmost, false);
        }
        state.was_topmost = state
            .view
            .load()
            .is_some_and(|view| unsafe { is_topmost(&view, event) });
        state.was_topmost
    })
}

extern "C-unwind" fn mouse_moved(observer: &AnyObject, _: Sel, event: &AnyObject) {
    if should_forward(observer, event, false) {
        unsafe {
            let _: () = msg_send![super(observer, observer.class().superclass().unwrap()), mouseMoved: event];
        }
    }
}

extern "C-unwind" fn mouse_entered(observer: &AnyObject, _: Sel, event: &AnyObject) {
    if should_forward(observer, event, false) {
        unsafe {
            let _: () = msg_send![super(observer, observer.class().superclass().unwrap()), mouseEntered: event];
        }
    }
}

extern "C-unwind" fn mouse_exited(observer: &AnyObject, _: Sel, event: &AnyObject) {
    if should_forward(observer, event, true) {
        unsafe {
            let _: () = msg_send![super(observer, observer.class().superclass().unwrap()), mouseExited: event];
        }
    }
}

/// Must be called through with_webview, on the AppKit main thread, with the host WKWebView.
pub unsafe fn isolate_host_mouse_tracking(view: *mut AnyObject) {
    let Some(view) = view.as_ref() else { return };
    let Some(base) = AnyClass::get(c"WKMouseTrackingObserver") else {
        return;
    };
    // Newer WebKit already implements this fix; leave its observer completely untouched.
    if base
        .instance_method(sel!(updateViewIsTopmostAtMouseLocation:))
        .is_some()
    {
        return;
    }
    let areas: *mut AnyObject = msg_send![view, trackingAreas];
    let count: usize = msg_send![areas, count];
    HOSTS.with(|hosts| {
        hosts
            .borrow_mut()
            .retain(|_, state| state.view.load().is_some())
    });
    for index in 0..count {
        let area: *mut AnyObject = msg_send![areas, objectAtIndex: index];
        let owner: *mut AnyObject = msg_send![area, owner];
        let Some(owner) = owner.as_ref() else {
            continue;
        };
        // Do not intercept AppKit gesture, tooltip, or application-owned tracking areas.
        if owner.class() != base {
            continue;
        }
        HOSTS.with(|hosts| {
            hosts.borrow_mut().insert(
                owner as *const AnyObject as usize,
                TrackingState {
                    view: Weak::new(view),
                    was_topmost: false,
                },
            );
        });
        AnyObject::set_class(owner, observer_class(base));
    }
}
