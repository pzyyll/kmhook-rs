//! Copyright: 2024 Lizc. All rights reserved.
//! License: MIT License
//! You may obtain a copy of the License at https://opensource.org/licenses/MIT
//!
//! Author: Lizc
//! Created Data: 2024-09-29
//!
//! Description: add msg listener
use super::event_loop::{EventLoop, EVENT_LOOP_MANAGER};
use super::worker::{Worker, WorkerMsg};
use super::WM_USER_RECHECK_HOOK;
use crate::consts;
use crate::types::{EventListener, JoinHandleType};
use crate::types::{EventType, KeyState, Shortcut, ID};
use crate::utils::gen_id;

use std::collections::HashMap;
use std::result::Result;
use std::sync::{Arc, Mutex};
use std::time::Instant;

type FnEvent = Arc<Box<dyn Fn(EventType) + Send + Sync + 'static>>;
type FnShourtcut = Arc<Box<dyn Fn() + Send + Sync + 'static>>;

#[derive(Clone)]
struct FnShourtcutTrigger {
    cb: FnShourtcut,
}

impl FnShourtcutTrigger {
    fn from_fn<F>(cb: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            cb: Arc::new(Box::new(cb)),
        }
    }
}

#[derive(Debug)]
struct ShortcutTriggerInfo {
    trigger: u32,
    last_trigger_time: Instant,
}

impl ShortcutTriggerInfo {
    fn new() -> Self {
        Self {
            trigger: 0,
            last_trigger_time: Instant::now(),
        }
    }

    fn reset(&mut self) {
        self.trigger = 0;
        self.last_trigger_time = Instant::now();
    }

    fn increase(&mut self) {
        self.trigger += 1;
        self.last_trigger_time = Instant::now();
    }
}

pub struct Listener {
    listener_event_loop: Mutex<Option<Arc<EventLoop>>>,
    worker: Mutex<Option<Arc<Worker>>>,
    event_map: Mutex<HashMap<ID, (EventType, FnEvent)>>,
    shortcut_map: Mutex<HashMap<ID, (Shortcut, FnShourtcutTrigger)>>,
    shortcut_ex_map: Mutex<HashMap<ID, Vec<ID>>>,
}

impl Listener {
    pub(crate) fn get_worker(&self) -> Option<Arc<Worker>> {
        self.worker.lock().unwrap().clone()
    }

    fn get_event_loop(&self) -> Option<Arc<EventLoop>> {
        self.listener_event_loop.lock().unwrap().clone()
    }

    fn filter_events(&self, event_type: &EventType) -> Vec<(EventType, FnEvent)> {
        let binding = self.event_map.lock().unwrap();
        binding
            .iter()
            .filter_map(|(_, (et, cb))| {
                if matches!(et, EventType::All)
                    || std::mem::discriminant(et) == std::mem::discriminant(event_type)
                {
                    Some((et.clone(), cb.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn filter_shortcut(&self, et: &EventType) -> Option<Vec<FnShourtcut>> {
        match et {
            EventType::KeyboardEvent(Some(key_info)) => {
                if key_info.state != KeyState::Pressed {
                    return None;
                }
                let mut result: Vec<FnShourtcut> = Vec::new();
                if let Some(keyboard_state) = &key_info.keyboard_state {
                    // println!("filter shortcut: {:?}", keyboard_state);
                    let binding = self.shortcut_map.lock().unwrap();
                    // let usb_input = keyboard_state.clone().usb_input_report().to_vec();
                    for (_, (shortcut, trigger)) in binding.iter() {
                        // println!("filter shortcut check: {:?}", shortcut);
                        if shortcut.is_match(keyboard_state) {
                            // Check if the modifier key is pressed, and when used with other keys,
                            // the last key pressed must not be a modifier key.
                            if shortcut.has_modifier()
                                & shortcut.has_normal_key()
                                & key_info.key_id.is_modifier()
                            {
                                continue;
                            }
                            result.push(trigger.cb.clone());
                        }
                    }
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    fn on_event(&self, event_type: EventType) {
        #[cfg(feature = "Debug")]
        println!(
            "{:?} on_event {:?}",
            std::thread::current().id(),
            event_type
        );

        let events = self.filter_events(&event_type);
        for (et, cb) in events.iter() {
            if matches!(et, EventType::All)
                || std::mem::discriminant(et) == std::mem::discriminant(&event_type)
            {
                cb(event_type.clone());
            }
        }

        if let Some(cbs) = self.filter_shortcut(&event_type) {
            for cb in cbs {
                cb();
            }
        }

        #[cfg(feature = "Debug")]
        println!(
            "{:?} event_type: {:?}\n ----------------on_event Finish ",
            std::thread::current().id(),
            event_type
        );
    }

    fn gen_id(&self) -> ID {
        gen_id()
    }

    fn post_recheck_hook(&self) {
        self.listener_event_loop
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .post_msg_to_loop(WM_USER_RECHECK_HOOK);
    }

    pub fn has_keyboard_event(&self) -> bool {
        {
            if !self.shortcut_map.lock().unwrap().is_empty() {
                return true;
            }
        }

        let binding = self.event_map.lock().unwrap();
        for (_, (et, _)) in binding.iter() {
            if matches!(et, EventType::KeyboardEvent(_) | EventType::All) {
                return true;
            }
        }
        false
    }

    pub fn has_mouse_event(&self) -> bool {
        let binding = self.event_map.lock().unwrap();
        for (_, (et, _)) in binding.iter() {
            if matches!(et, EventType::MouseEvent(_) | EventType::All) {
                return true;
            }
        }
        false
    }

    fn register_shortcut_callback(
        &self,
        shortcut: &str,
        trigger: FnShourtcutTrigger,
    ) -> Result<usize, String> {
        let id = self.gen_id();
        {
            let shortcut = Shortcut::from_str(shortcut)?;
            let mut binding = self.shortcut_map.lock().map_err(|e| e.to_string())?;
            for (_, (sc, _)) in binding.iter() {
                // println!("sc usb_input: {:?}", sc.usb_input());
                // println!("shortcut usb_input: {:?}", shortcut.usb_input());
                if *sc == shortcut {
                    return Err("Shortcut already exists".to_string());
                }
            }
            binding.insert(id, (shortcut, trigger));
        }
        Ok(id)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        println!("Listener drop");
        self.shutdown();
    }
}

impl EventListener for Listener {
    fn new() -> Arc<Self> {
        let listener = Self {
            listener_event_loop: Mutex::new(None),
            event_map: Mutex::new(HashMap::new()),
            shortcut_map: Mutex::new(HashMap::new()),
            worker: Mutex::new(None),
            shortcut_ex_map: Mutex::new(HashMap::new()),
        };
        let rc = Arc::new(listener);
        rc.listener_event_loop
            .lock()
            .unwrap()
            .replace(EVENT_LOOP_MANAGER.lock().unwrap().new_event_loop(&rc));
        rc.worker.lock().unwrap().replace(Arc::new(Worker::new()));
        rc
    }

    /// `work_thread`:
    /// Handle event callbacks in a separate thread. Default is `true`.
    /// return: `Option<JoinHandleType>` if `work_thread` is `true`, else `None`.
    fn startup(self: &Arc<Self>, work_thread: Option<bool>) -> Option<JoinHandleType> {
        if let Some(event_loop) = self.get_event_loop().as_ref() {
            event_loop.run_with_thread();
        }

        if let Some(w) = self.get_worker() {
            let _self = self.clone();
            w.run(
                move |event_type| {
                    _self.on_event(event_type);
                },
                work_thread,
            )
        } else {
            None
        }
    }

    fn shutdown(&self) {
        self.del_all_events();
        if let Some(worker) = self.get_worker() {
            worker.post_msg(WorkerMsg::Stop);
        }
        if let Some(event_loop) = self.listener_event_loop.lock().unwrap().as_ref() {
            event_loop.stop();
        }
    }

    fn add_event_listener<F>(&self, cb: F, event_type: Option<EventType>) -> Result<ID, String>
    where
        F: Fn(EventType) + Send + Sync + 'static,
    {
        let id = self.gen_id();
        let et = event_type.unwrap_or(EventType::All);
        self.event_map
            .lock()
            .unwrap()
            .insert(id, (et, Arc::new(Box::new(cb))));
        self.post_recheck_hook();
        Ok(id)
    }

    fn add_global_shortcut<F>(&self, shortcut: &str, cb: F) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let id = self.register_shortcut_callback(shortcut, FnShourtcutTrigger::from_fn(cb))?;
        self.post_recheck_hook();
        Ok(id)
    }

    fn add_global_shortcut_trigger<F>(
        &self,
        shortcut: &str,
        cb: F,
        trigger: u32,
        internal: Option<u32>,
    ) -> std::result::Result<ID, String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let trigger_info = Arc::new(Mutex::new(ShortcutTriggerInfo::new()));
        let next_internal = internal.unwrap_or(consts::DEFAULT_SHORTCUT_TRIGGER_INTERVAL) as u128;

        self.add_global_shortcut(shortcut, move || {
            #[cfg(feature = "Debug")]
            println!("global_shortcut trigger: {:?}", Instant::now());

            let need_trigger = {
                let mut mtrigger_info = trigger_info.lock().unwrap();

                let elapsed = mtrigger_info.last_trigger_time.elapsed().as_millis();
                #[cfg(feature = "Debug")]
                println!(
                    "trigger times: {:?}, elapsed: {:?}",
                    mtrigger_info.trigger, elapsed
                );

                if mtrigger_info.trigger == 0 || elapsed < next_internal {
                    mtrigger_info.increase();
                } else {
                    mtrigger_info.reset();
                    mtrigger_info.increase();
                }
                if mtrigger_info.trigger >= trigger {
                    mtrigger_info.reset();
                    true
                } else {
                    false
                }
            };
            if need_trigger {
                cb();
                #[cfg(feature = "Debug")]
                println!(
                    "------------------------Trigger------------------------{:?}",
                    Instant::now()
                );
            }
        })
    }

    fn del_all_events(&self) {
        self.event_map.lock().unwrap().clear();
        self.shortcut_map.lock().unwrap().clear();
        self.post_recheck_hook();
    }

    fn del_event_by_id(&self, id: ID) {
        let ids = self.shortcut_ex_map.lock().unwrap().remove(&id);
        if let Some(ids) = ids {
            for id in ids {
                self.shortcut_map.lock().unwrap().remove(&id);
            }
        }
        self.event_map.lock().unwrap().remove(&id);
        self.shortcut_map.lock().unwrap().remove(&id);
        self.post_recheck_hook();
        println!("del_event_by_id finish {:?}", id);
    }
}
