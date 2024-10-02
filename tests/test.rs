use std::{cell::RefCell, thread};

struct L {
    var1: std::rc::Rc<RefCell<i32>>,
}

impl L {
    fn ps(&self) {
        println!("{:?}", self.var1.borrow());

        let f = move || {
            println!("z {:?}", self.var1.borrow());
        };

        f();
    }
}

#[test]
fn test_get_current_thread_id() {
    use windows::Win32::System::Threading::GetCurrentThreadId;
    println!("{:?}", unsafe { GetCurrentThreadId() });
    std::cell::RefCell::new(1);
    // get current thread id by rust

    println!("{:?}", thread::current().id());

    let l = L {
        var1: std::rc::Rc::new(RefCell::new(1)),
    };

    l.ps();
}
