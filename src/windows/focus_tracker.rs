// ABOUTME: Windows 平台下的窗口焦点跟踪器
// ABOUTME: 提供可靠的窗口焦点检测和历史记录功能

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::time::SystemTime;
// use windows::core::PCWSTR;
pub use windows::Win32::Foundation::HWND;
// use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, EVENT_SYSTEM_FOREGROUND,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

// Thread-safe wrapper for HWINEVENTHOOK
#[derive(Debug)]
struct ThreadSafeHook(HWINEVENTHOOK);

unsafe impl Send for ThreadSafeHook {}
unsafe impl Sync for ThreadSafeHook {}

impl ThreadSafeHook {
    fn new(hook: HWINEVENTHOOK) -> Self {
        Self(hook)
    }

    fn get(&self) -> HWINEVENTHOOK {
        self.0
    }

    fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
}

// Thread-safe wrapper for HWND
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadSafeHwnd(HWND);

unsafe impl Send for ThreadSafeHwnd {}
unsafe impl Sync for ThreadSafeHwnd {}

impl ThreadSafeHwnd {
    pub fn new(hwnd: HWND) -> Self {
        Self(hwnd)
    }

    pub fn get(&self) -> HWND {
        self.0
    }

    pub fn is_invalid(&self) -> bool {
        self.0.is_invalid()
    }
}

pub type FocusChangeCallback = Arc<dyn Fn(&FocusInfo) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct FocusInfo {
    pub hwnd: ThreadSafeHwnd,
    pub process_id: u32,
    pub thread_id: u32,
    pub window_title: String,
    pub timestamp: SystemTime,
}

impl FocusInfo {
    pub fn new(hwnd: HWND) -> Self {
        let mut process_id = 0u32;
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };

        let window_title = unsafe {
            let mut buffer = [0u16; 256];
            let len = GetWindowTextW(hwnd, &mut buffer);
            if len > 0 {
                String::from_utf16_lossy(&buffer[..len as usize])
            } else {
                String::new()
            }
        };

        Self {
            hwnd: ThreadSafeHwnd::new(hwnd),
            process_id,
            thread_id,
            window_title,
            timestamp: SystemTime::now(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.hwnd.is_invalid()
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd.get()
    }
}

pub struct FocusTracker {
    current_focus: Arc<Mutex<Option<FocusInfo>>>,
    focus_history: Arc<Mutex<VecDeque<FocusInfo>>>,
    callbacks: Arc<Mutex<Vec<FocusChangeCallback>>>,
    hook_handle: Arc<Mutex<Option<ThreadSafeHook>>>,
    max_history: usize,
}

impl FocusTracker {
    pub fn new(max_history: Option<usize>) -> Arc<Self> {
        let tracker = Arc::new(Self {
            current_focus: Arc::new(Mutex::new(None)),
            focus_history: Arc::new(Mutex::new(VecDeque::new())),
            callbacks: Arc::new(Mutex::new(Vec::new())),
            hook_handle: Arc::new(Mutex::new(None)),
            max_history: max_history.unwrap_or(50),
        });

        // 立即获取当前焦点窗口
        tracker.update_current_focus();
        tracker
    }

    fn update_current_focus(&self) {
        let hwnd = unsafe { GetForegroundWindow() };
        if !hwnd.is_invalid() {
            let focus_info = FocusInfo::new(hwnd);
            self.set_current_focus(focus_info);
        }
    }

    pub(crate) fn set_current_focus(&self, focus_info: FocusInfo) {
        // 更新当前焦点
        {
            let mut current = self.current_focus.lock().unwrap();
            *current = Some(focus_info.clone());
        }

        // 添加到历史记录
        {
            let mut history = self.focus_history.lock().unwrap();
            history.push_front(focus_info.clone());

            // 保持历史记录大小限制
            while history.len() > self.max_history {
                history.pop_back();
            }
        }

        // 触发回调
        let callbacks = self.callbacks.lock().unwrap();
        for callback in callbacks.iter() {
            callback(&focus_info);
        }
    }

    pub fn start_tracking(self: &Arc<Self>) -> Result<(), String> {
        // 在全局静态变量中存储tracker的弱引用
        unsafe {
            TRACKER_WEAK_REF = Some(Arc::downgrade(self));
        }

        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(Self::win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };

        if hook.is_invalid() {
            return Err("Failed to set windows event hook".to_string());
        }

        *self.hook_handle.lock().unwrap() = Some(ThreadSafeHook::new(hook));
        Ok(())
    }

    pub fn stop_tracking(&self) {
        if let Some(hook) = self.hook_handle.lock().unwrap().take() {
            unsafe {
                let _ = UnhookWinEvent(hook.get());
                TRACKER_WEAK_REF = None;
            }
        }
    }

    unsafe extern "system" fn win_event_proc(
        _h_win_event_hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
        _event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _dw_event_thread: u32,
        _dwms_event_time: u32,
    ) {
        println!("Focus changed: HWND={:?}", hwnd);
        if !hwnd.is_invalid() {
            let weak_ref_ptr = &raw const TRACKER_WEAK_REF;
            let weak_ref_opt = unsafe { &*weak_ref_ptr };
            if let Some(weak_ref) = weak_ref_opt {
                if let Some(tracker) = weak_ref.upgrade() {
                    let focus_info = FocusInfo::new(hwnd);
                    tracker.set_current_focus(focus_info);
                }
            }
        }
    }

    pub fn get_current_focus(&self) -> Option<FocusInfo> {
        self.current_focus.lock().unwrap().clone()
    }

    pub fn get_previous_focus(&self) -> Option<FocusInfo> {
        let history = self.focus_history.lock().unwrap();
        history.get(1).cloned()
    }

    pub fn get_focus_history(&self) -> Vec<FocusInfo> {
        self.focus_history.lock().unwrap().clone().into()
    }

    pub fn get_focus_before_window(&self, target_hwnd: HWND) -> Option<FocusInfo> {
        let history = self.focus_history.lock().unwrap();
        let mut found_target = false;
        let target_safe_hwnd = ThreadSafeHwnd::new(target_hwnd);

        for focus_info in history.iter() {
            if found_target && focus_info.hwnd != target_safe_hwnd {
                return Some(focus_info.clone());
            }
            if focus_info.hwnd == target_safe_hwnd {
                found_target = true;
            }
        }
        None
    }

    pub fn add_focus_change_callback(&self, callback: FocusChangeCallback) {
        self.callbacks.lock().unwrap().push(callback);
    }

    pub fn clear_history(&self) {
        self.focus_history.lock().unwrap().clear();
    }

    pub fn find_focus_by_title(&self, title_pattern: &str) -> Vec<FocusInfo> {
        let history = self.focus_history.lock().unwrap();
        history
            .iter()
            .filter(|info| info.window_title.contains(title_pattern))
            .cloned()
            .collect()
    }

    pub fn find_focus_by_process(&self, process_id: u32) -> Vec<FocusInfo> {
        let history = self.focus_history.lock().unwrap();
        history
            .iter()
            .filter(|info| info.process_id == process_id)
            .cloned()
            .collect()
    }
}

impl Drop for FocusTracker {
    fn drop(&mut self) {
        self.stop_tracking();
    }
}

// 静态变量来存储tracker的弱引用
static mut TRACKER_WEAK_REF: Option<Weak<FocusTracker>> = None;

// 全局的焦点跟踪器实例管理
use lazy_static::lazy_static;

lazy_static! {
    static ref GLOBAL_FOCUS_TRACKER: Arc<Mutex<Option<Arc<FocusTracker>>>> =
        Arc::new(Mutex::new(None));
}

pub fn get_global_focus_tracker() -> Option<Arc<FocusTracker>> {
    GLOBAL_FOCUS_TRACKER.lock().unwrap().clone()
}

pub fn initialize_global_focus_tracker(
    max_history: Option<usize>,
) -> Result<Arc<FocusTracker>, String> {
    let tracker = FocusTracker::new(max_history);
    tracker.start_tracking()?;

    *GLOBAL_FOCUS_TRACKER.lock().unwrap() = Some(tracker.clone());
    Ok(tracker)
}

pub fn shutdown_global_focus_tracker() {
    if let Some(tracker) = GLOBAL_FOCUS_TRACKER.lock().unwrap().take() {
        tracker.stop_tracking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_tracker_creation() {
        let tracker = FocusTracker::new(Some(10));
        assert!(tracker.get_current_focus().is_some());
    }

    #[test]
    fn test_focus_history() {
        let tracker = FocusTracker::new(Some(5));
        let history = tracker.get_focus_history();
        assert!(history.len() <= 5);
    }

    #[test]
    fn test_global_tracker() {
        let result = initialize_global_focus_tracker(Some(20));
        assert!(result.is_ok());

        let tracker = get_global_focus_tracker();
        assert!(tracker.is_some());

        shutdown_global_focus_tracker();
        let tracker_after_shutdown = get_global_focus_tracker();
        assert!(tracker_after_shutdown.is_none());
    }
}
