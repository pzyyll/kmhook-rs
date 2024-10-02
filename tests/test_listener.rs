use kmhook_rs::EventListener;

#[test]
fn test_event_listener() {
    let listener = kmhook_rs::Listener::new();
    listener.startup(Some(true));
    // listener.as_ref().shutdown();
}