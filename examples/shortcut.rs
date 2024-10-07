#![allow(warnings)]
use std::time::Duration;

use kmhook::types::*;
use kmhook::*;

use kmhook::enginer as listener;

fn main() {
    listener::add_global_shortcut(
        "Ctrl+Shift+A",
        || println!("Ctrl + Shift + A"),
    );

    listener::add_global_shortcut(
        "Ctrl+C+V",
        || println!("Ctrl + C + V"),
    );

    listener::add_global_shortcut(
        "Ctrl+V+C",
        || println!("Ctrl + V + C"),
    );

    // listener.add_global_shortcut(Shortcut::new(vec![KeyMappingId::AltLeft]).unwrap(), || {
    //     println!("Alt Left")
    // });

    listener::add_global_shortcut_trigger(
        "Alt",
        || {
            println!(
                "》》》》》》》》》》》Triple Alt {:?}",
                std::thread::current().id()
            );
            std::thread::sleep(Duration::from_millis(1000));
        },
        2,
        Some(400),
    );

    listener::add_global_shortcut_trigger(
        "Ctrl+C",
        || println!("Double Ctrl + C"),
        2,
        Some(400),
    );

    // Illegal shortcut key
    // listener.add_global_shortcut(
    //     Shortcut::new(vec![KeyMappingId::ControlLeft, KeyMappingId::UsV, KeyMappingId::UsC, KeyMappingId::UsC]).expect("Failed to create shortcut"),
    //     || println!("Ctrl + V + C + C"),
    // );

    // listener.startup(None);
    // work on thread
    if let Some(join) = listener::startup(Some(true)) {
        join.join().unwrap();
    }
}
