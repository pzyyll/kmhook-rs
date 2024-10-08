#![allow(warnings)]

use kmhook_rs::{
    types::{EventListener, EventType, KeyMappingId, KeyId, MouseButton},
    Listener,
};
use std::sync::{Arc, Mutex};

fn main() {
    let listener = Listener::new();

    let l = listener.clone();
    let result = listener.add_event_listener(
        move |event_type: EventType| match event_type {
            EventType::KeyboardEvent(Some(info)) => {
                println!("KeyboardEvent {:?}", info);
                println!(
                    "KeyboardState {:?}",
                    info.keyboard_state.unwrap().usb_input_report()
                );
                if info.key_id == KeyId::from(KeyMappingId::UsA) {
                    println!("Pressed A");
                } else if info.key_id == KeyId::from(KeyMappingId::Escape) {
                    println!("Pressed Escape");
                    l.as_ref().shutdown();
                }
            }
            _ => {}
        },
        Some(EventType::KeyboardEvent(None)),
    );
    println!("{:?}", result);

    listener.add_event_listener(
        move |event_type| match event_type {
            EventType::MouseEvent(Some(info)) => {
                println!("Mouse Button {:?}", info.button);
                println!("Mouse Position {:?}", info.pos);
                println!("Mouse State {:?}", info.relative_pos);
            }
            _ => {}
        },
        Some(EventType::MouseEvent(None)),
    );

    if let Some(join) = listener.startup(Some(true)) {
        join.join().unwrap();
    }
    // listener.startup(None);
}
