use std::ptr;
use libc::{c_void, c_int};

extern {
    pub fn panbridge_init(argc: *const c_void, argv: *const c_void);
    pub fn panbridge_exit();
    pub fn triple(x: c_int) -> c_int;
}

fn main() {
    unsafe {
        let argc = ptr::null();
        let argv = ptr::null();
        panbridge_init(argc, argv);
    }
    println!("Hello, world! {}", unsafe { triple(3) });
    unsafe { panbridge_exit(); }
}
