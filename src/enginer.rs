use crate::types::{EventListener, EventType, JoinHandleType, ID};
use crate::Listener;
use lazy_static::lazy_static;
use std::sync::Arc;

#[cfg(target_os = "windows")]
use crate::windows::focus_tracker::{
    get_global_focus_tracker, initialize_global_focus_tracker, shutdown_global_focus_tracker,
    FocusInfo, FocusTracker, HWND,
};

lazy_static! {
    static ref LISTENER: Arc<Listener> = Listener::new();
}

pub fn add_global_shortcut<F>(shortcut: &str, cb: F) -> std::result::Result<ID, String>
where
    F: Fn() + Send + Sync + 'static,
{
    LISTENER.add_global_shortcut(shortcut, cb)
}

pub fn add_global_shortcut_trigger<F>(
    shortcut: &str,
    cb: F,
    trigger: u32,
    internal: Option<u32>,
) -> std::result::Result<ID, String>
where
    F: Fn() + Send + Sync + 'static,
{
    LISTENER.add_global_shortcut_trigger(shortcut, cb, trigger, internal)
}

pub fn del_event_by_id(id: ID) {
    LISTENER.del_event_by_id(id);
}

pub fn del_all_events() {
    LISTENER.del_all_events();
}

pub fn add_event_listener<F>(
    cb: F,
    event_type: Option<EventType>,
) -> std::result::Result<ID, String>
where
    F: Fn(EventType) + Send + Sync + 'static,
{
    LISTENER.add_event_listener(cb, event_type)
}

pub fn startup(work_thread: Option<bool>) -> Option<JoinHandleType> {
    LISTENER.startup(work_thread)
}

pub fn shutdown() {
    LISTENER.shutdown();
}

// 焦点跟踪相关的公共API
#[cfg(target_os = "windows")]
pub fn start_focus_tracking(max_history: Option<usize>) -> Result<Arc<FocusTracker>, String> {
    initialize_global_focus_tracker(max_history)
}

#[cfg(target_os = "windows")]
pub fn stop_focus_tracking() {
    shutdown_global_focus_tracker();
}

#[cfg(target_os = "windows")]
pub fn get_focus_tracker() -> Option<Arc<FocusTracker>> {
    get_global_focus_tracker()
}

#[cfg(target_os = "windows")]
pub fn get_current_focus() -> Option<FocusInfo> {
    get_global_focus_tracker()?.get_current_focus()
}

#[cfg(target_os = "windows")]
pub fn get_previous_focus() -> Option<FocusInfo> {
    get_global_focus_tracker()?.get_previous_focus()
}

#[cfg(target_os = "windows")]
pub fn get_focus_before_window(target_hwnd: HWND) -> Option<FocusInfo> {
    get_global_focus_tracker()?.get_focus_before_window(target_hwnd)
}
