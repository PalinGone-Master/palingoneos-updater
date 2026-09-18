// Copyright 2022 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file).

/// ## Compatibility with async runtimes
///
/// The following only applies on Linux/Unix:
///
/// While this crate's API is purely blocking, it internally spawns asynchronous tasks on an executor.
///
/// - If you use tokio, make sure to enable the `tokio` feature of this crate.
/// - If you use another async runtime or if you don't use one at all, the default feature will suit your needs.

#[cfg(all(
    feature = "accesskit_unix",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    not(feature = "async-io"),
    not(feature = "tokio")
))]
compile_error!("Either \"async-io\" (default) or \"tokio\" feature must be enabled.");

#[cfg(all(
    feature = "accesskit_unix",
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    feature = "async-io",
    feature = "tokio"
))]
compile_error!(
    "Both \"async-io\" (default) and \"tokio\" features cannot be enabled at the same time."
);

#[cfg(all(not(feature = "rwh_05"), not(feature = "rwh_06")))]
compile_error!("Either \"rwh_06\" (default) or \"rwh_05\" feature must be enabled.");

#[cfg(all(feature = "rwh_05", feature = "rwh_06"))]
compile_error!(
    "Both \"rwh_06\" (default) and \"rwh_05\" features cannot be enabled at the same time."
);

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use winit::{
    event::WindowEvent as WinitWindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[cfg(feature = "rwh_05")]
#[allow(unused)]
use rwh_05 as raw_window_handle;
#[cfg(feature = "rwh_06")]
#[allow(unused)]
use rwh_06 as raw_window_handle;

mod platform_impl;

#[derive(Debug)]
pub struct Event {
    pub window_id: WindowId,
    pub window_event: WindowEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    InitialTreeRequested,
    ActionRequested(ActionRequest),
    AccessibilityDeactivated,
}

pub struct Adapter {
    inner: platform_impl::Adapter,
}

impl Adapter {
    /// Creates a new AccessKit adapter for a winit window. This must be done
    /// before the window is shown for the first time. This means that you must
    /// use [`winit::window::WindowAttributes::with_visible`] to make the window
    /// initially invisible, then create the adapter, then show the window.
    ///
    /// Use this if you want to provide your own AccessKit handler callbacks
    /// rather than dispatching requests through the winit event loop. This is
    /// especially useful for the activation handler, because depending on
    /// your application's architecture, implementing the handler directly may
    /// allow you to return an initial tree synchronously, rather than requiring
    /// some platform adapters to use a placeholder tree until you send
    /// the first update. However, remember that each of these handlers may be
    /// called on any thread, depending on the underlying platform adapter.
    ///
    /// # Panics
    ///
    /// Panics if the window is already visible.
    pub fn with_direct_handlers(
        event_loop: &dyn ActiveEventLoop,
        window: &dyn Window,
        activation_handler: impl 'static + ActivationHandler + Send,
        action_handler: impl 'static + ActionHandler + Send,
        deactivation_handler: impl 'static + DeactivationHandler + Send,
    ) -> Self {
        if window.is_visible() == Some(true) {
            panic!("The AccessKit winit adapter must be created before the window is shown (made visible) for the first time.");
        }

        let inner = platform_impl::Adapter::new(
            event_loop,
            window,
            activation_handler,
            action_handler,
            deactivation_handler,
        );
        Self { inner }
    }

    /// Allows reacting to window events.
    ///
    /// This must be called whenever a new window event is received
    /// and before it is handled by the application.
    pub fn process_event(&mut self, window: &dyn Window, event: &WinitWindowEvent) {
        self.inner.process_event(window, event);
    }

    /// If and only if the tree has been initialized, call the provided function
    /// and apply the resulting update. Note: If the caller's implementation of
    /// [`ActivationHandler::request_initial_tree`] initially returned `None`,
    /// or if the caller created the adapter using [`EventLoopProxy`], then
    /// the [`TreeUpdate`] returned by the provided function must contain
    /// a full tree.
    pub fn update_if_active(&mut self, updater: impl FnOnce() -> TreeUpdate) {
        self.inner.update_if_active(updater);
    }
}
