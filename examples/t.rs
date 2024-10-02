use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex, Weak,
};

struct Worker {
    msg_sender: Mutex<Option<Sender<Option<String>>>>,
    listener: Weak<Listener>,
}

impl Worker {
    fn new(listener: Arc<Listener>) -> Arc<Self> {
        Arc::new(Worker {
            msg_sender: Mutex::new(None),
            listener: Arc::downgrade(&listener),
        })
    }

    fn set_sender(&self, sender: Sender<Option<String>>) {
        let mut locked_sender = self.msg_sender.lock().unwrap();
        *locked_sender = Some(sender);
    }

    fn send_event(&self, event: Option<String>) {
        if let Some(sender) = self.msg_sender.lock().unwrap().as_ref() {
            let _ = sender.send(event);
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        println!("Worker is being dropped. ");
    }
}

pub struct Listener {
    worker: Mutex<Option<Arc<Worker>>>,
}

impl Listener {
    fn new() -> Arc<Self> {
        Arc::new(Listener {
            worker: Mutex::new(None),
        })
    }

    fn set_worker(&self, worker: Arc<Worker>) {
        let mut locked_worker = self.worker.lock().unwrap();
        *locked_worker = Some(worker);
    }

    fn get_worker(&self) -> Option<Arc<Worker>> {
        self.worker.lock().unwrap().clone()
    }

    fn recv(self: &Arc<Self>, rx: Receiver<Option<String>>) {
        // Check the message received
        let cb = move || {
            while let Ok(message) = rx.recv() {
                println!("Received message: {:?}", message);
            }
        };
        cb();
        ()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        println!("Listener is being dropped.");
    }
}

fn main() {
    let listener = Listener::new();
    let worker = Worker::new(listener.clone());

    let (tx, rx): (Sender<Option<String>>, Receiver<Option<String>>) = channel();
    worker.set_sender(tx);

    listener.set_worker(worker.clone());

    // Sending an event to verify everything works
    worker.send_event(Some("Hello from Worker!".to_string()));

    listener.recv(rx);

    // Drop the strong references explicitly to check behavior
    drop(worker);
    drop(listener);

    // At this point, the drop messages with stack traces should appear
}
