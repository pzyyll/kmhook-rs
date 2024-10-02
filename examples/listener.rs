use kmhook_rs::{
    types::{EventType, MouseButton},
    EventListener, Listener,
};

fn main() {
    let listener = Listener::new();

    let result = listener.add_event_listener(
        |event_type: EventType| {
            match event_type {
                EventType::KeyboardEvent(Some(info)) => {
                    println!("KeyboardEvent {:?}", info);
                }
                EventType::MouseEvent(Some(info)) => {
                    if let MouseButton::Left(flag) = info.button {
                        println!("Left {:?}", flag);
                    }
                }
                _ => {}
            }
        },
        None,
    );
    println!("{:?}", result);

    if let Some(join) = listener.startup(Some(true)) {
        join.join().unwrap();
    }
    // listener.startup(None);
}
