use crate::types::{EventListener, EventType, JoinHandleType, ID};
use crate::Listener;
use lazy_static::lazy_static;
use std::sync::Arc;

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
