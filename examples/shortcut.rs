#![allow(warnings)]
use kmhook_rs::types::*;
use kmhook_rs::*;

fn main() {
    let listener = Listener::new();

    listener.add_global_shortcut(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsA]).expect("Failed to create shortcut"),
        || println!("Ctrl + A"),
    );

    let result = listener.add_global_shortcut(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsA]).expect("Failed to create shortcut"),
        || println!("Ctrl + A"),
    );
    // Shortcut already exists
    assert_eq!(result.is_err(), true);

    listener.add_global_shortcut(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::ShiftLeft, KeyCode::UsA])
            .expect("Failed to create shortcut"),
        || println!("Ctrl + Shift + A"),
    );

    listener.add_global_shortcut(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsC, KeyCode::UsV])
            .expect("Failed to create shortcut"),
        || println!("Ctrl + C + V"),
    );

    listener.add_global_shortcut(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsV, KeyCode::UsC])
            .expect("Failed to create shortcut"),
        || println!("Ctrl + V + C"),
    );

    // listener.add_global_shortcut(Shortcut::new(vec![KeyCode::AltLeft]).unwrap(), || {
    //     println!("Alt Left")
    // });

    listener.add_global_shortcut_trigger(
        Shortcut::new(vec![KeyCode::AltLeft]).unwrap(),
        || println!("Triple Alt"),
        3,
        Some(400),
    );

    listener.add_global_shortcut_trigger(
        Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsC]).unwrap(),
        || println!("Double Ctrl + C"),
        2,
        Some(400),
    );

    // Illegal shortcut key
    // listener.add_global_shortcut(
    //     Shortcut::new(vec![KeyCode::ControlLeft, KeyCode::UsV, KeyCode::UsC, KeyCode::UsC]).expect("Failed to create shortcut"),
    //     || println!("Ctrl + V + C + C"),
    // );

    listener.startup(None);
    // work on thread
    // if let Some(join) = listener.startup(Some(true)) {
    //     join.join().unwrap();
    // }
}
