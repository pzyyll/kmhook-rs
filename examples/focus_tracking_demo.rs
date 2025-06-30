// ABOUTME: 焦点跟踪功能演示程序
// ABOUTME: 展示如何使用kmhook-rs的焦点跟踪API

use kmhook::enginer::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("开始焦点跟踪演示...");

    add_global_shortcut("Digit0", || {
        println!("触发 SendInput 到上一个窗口");
        std::thread::sleep(std::time::Duration::from_millis(150));
        send_input_to_prev_window("text from focus tracking demo");
    })?;

    enable_focus_tracker(true);

    // 启动监听器
    if let Some(join_handle) = startup(Some(true)) {
        // 等待监听器启动完成
        join_handle.join().expect("监听器线程异常退出");
    }

    Ok(())
}