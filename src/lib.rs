use std::{sync::Arc, thread::JoinHandle};

type JoinHandleType = JoinHandle<()>;

pub mod types;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::listener::Listener;

pub trait EventListener {
    fn new() -> Arc<Self>;
    fn add_global_shortcut<F>(
        &self,
        shortcut: types::Shortcut,
        cb: F,
    ) -> std::result::Result<types::ID, String>
    where
        F: Fn()  + Send + Sync + 'static;

    fn add_event_listener<F>(
        &self,
        cb: F,
        event_type: Option<types::EventType>,
    ) -> std::result::Result<types::ID, String>
    where
        F: Fn(types::EventType) + Send + Sync + 'static;

    fn del_event_by_id(&self, id: types::ID);
    fn del_all_events(&self);

    fn startup(self: &Arc<Self>, work_thread: Option<bool>) -> Option<JoinHandleType>;
    fn shutdown(&self);
}
