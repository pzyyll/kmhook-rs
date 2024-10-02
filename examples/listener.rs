#![allow(warnings)]

use kmhook_rs::{
    types::{EventListener, EventType, KeyCode, KeyId, MouseButton},
    Listener,
};
use std::sync::{Arc, Mutex};

fn main() {
    let listener = Listener::new();

    let l = listener.clone();
    let id = Arc::new(Mutex::new(0));
    let id2 = id.clone();
    let result = listener.add_event_listener(
        move |event_type: EventType| match event_type {
            EventType::KeyboardEvent(Some(info)) => {
                println!("KeyboardEvent {:?}", info);
                println!(
                    "KeyboardState {:?}",
                    info.keyboard_state.unwrap().usb_input_report()
                );
                if info.key_id == KeyId::from(KeyCode::UsA) {
                    println!("Pressed A");
                    // let _ = id2.lock().and_then(|op| {
                    //     l.as_ref().del_event_by_id(*op);
                    //     Ok(())
                    // });
                } else if info.key_id == KeyId::from(KeyCode::Escape) {
                    println!("Pressed Escape");
                    l.as_ref().shutdown();
                }
            }
            _ => {}
        },
        Some(EventType::KeyboardEvent(None)),
    );
    println!("{:?}", result);
    *id.lock().unwrap() = result.unwrap();

    if let Some(join) = listener.startup(Some(true)) {
        join.join().unwrap();
    }
    // listener.startup(None);
}
