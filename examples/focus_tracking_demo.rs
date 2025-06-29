// ABOUTME: 焦点跟踪功能演示程序
// ABOUTME: 展示如何使用kmhook-rs的焦点跟踪API

use kmhook::enginer::*;
use std::sync::Arc;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始焦点跟踪演示...");

    // 启动焦点跟踪
    let tracker = start_focus_tracking(Some(100))?;
    println!("焦点跟踪已启动");

    // 添加焦点变化回调
    tracker.add_focus_change_callback(Arc::new(|focus_info| {
        println!(
            "焦点变化: HWND={:?}, 进程ID={}, 标题='{}'",
            focus_info.hwnd, focus_info.process_id, focus_info.window_title
        );
    }));

    // 注册一个快捷键来查询当前焦点信息
    add_global_shortcut("Ctrl+Shift+F", || {
        if let Some(current) = get_current_focus() {
            println!("\n=== 当前焦点信息 ===");
            println!("窗口句柄: {:?}", current.hwnd);
            println!("进程ID: {}", current.process_id);
            println!("线程ID: {}", current.thread_id);
            println!("窗口标题: '{}'", current.window_title);
            println!("时间戳: {:?}", current.timestamp);
        } else {
            println!("没有获取到当前焦点信息");
        }

        // 显示焦点历史
        if let Some(tracker) = get_focus_tracker() {
            let history = tracker.get_focus_history();
            println!("\n=== 焦点历史 (最近{}个) ===", history.len().min(5));
            for (i, focus) in history.iter().take(5).enumerate() {
                println!(
                    "{}. HWND={:?}, 标题='{}'",
                    i + 1,
                    focus.hwnd,
                    focus.window_title
                );
            }
        }
    })?;

    // 注册一个快捷键来查询前一个焦点
    add_global_shortcut("Ctrl+Shift+P", || {
        if let Some(previous) = get_previous_focus() {
            println!("\n=== 前一个焦点信息 ===");
            println!("窗口句柄: {:?}", previous.hwnd);
            println!("窗口标题: '{}'", previous.window_title);
        } else {
            println!("没有前一个焦点信息");
        }
    })?;

    // 注册退出快捷键
    add_global_shortcut("Ctrl+Shift+Q", || {
        println!("退出程序...");
        stop_focus_tracking();
        std::process::exit(0);
    })?;

    println!("焦点跟踪演示已启动!");
    println!("使用快捷键:");
    println!("  Ctrl+Shift+F - 查看当前焦点信息和历史");
    println!("  Ctrl+Shift+P - 查看前一个焦点信息");
    println!("  Ctrl+Shift+Q - 退出程序");
    println!("请切换不同的窗口来测试焦点跟踪功能...");

    // 启动事件循环
    startup(Some(true)).expect("Failed to start event loop");

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
