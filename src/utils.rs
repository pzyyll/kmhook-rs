use crate::types::ID;

pub fn gen_id() -> ID {
    static mut ID: ID = 0;
    unsafe {
        ID += 1;
        ID
    }
}
