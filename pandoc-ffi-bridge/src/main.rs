// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use libc::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ops::Drop;
use std::ptr;

extern "C" {
    fn panbridge_init();
    fn panbridge_exit();
    fn get_hso(hso: &mut HsOwnedString);
    fn stable_ptr_drop(ptr: *const c_void);
    fn tiny_parse(a: *const c_char, hso: &mut HsOwnedString);
}

#[derive(Debug)]
#[repr(C)]
struct StablePtr<T>(*const c_void, PhantomData<T>);
impl<T> Drop for StablePtr<T> {
    fn drop(&mut self) {
        unsafe { stable_ptr_drop(self.0) }
    }
}

#[repr(C)]
#[derive(Debug)]
struct HsOwnedString {
    string: *const c_char,
    // This is technically a token that can be converted to a pointer to a ByteString,
    // but we don't know what that is in Rust and don't need to know, so () will do.
    stable: StablePtr<()>,
}

impl HsOwnedString {
    fn new() -> Self {
        HsOwnedString {
            stable: StablePtr(ptr::null(), Default::default()),
            string: ptr::null(),
        }
    }
    fn as_ref(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.string) }
    }
}

fn main() {
    unsafe {
        panbridge_init();

        let mut hso = HsOwnedString::new();
        println!("input a string please");
        get_hso(&mut hso);
        println!("{:?}", hso);
        let cstr = hso.as_ref();
        println!("Haskell lent us a CStr: {:?}", cstr);
        println!("Here it is with unicode decoded again: {:?}", cstr.to_str());

        let arg = CString::new("100").expect("it's a string");
        tiny_parse(arg.as_ptr(), &mut hso);
        let cstr = hso.as_ref();
        println!("tiny parse returned: {:?}", cstr);

        panbridge_exit();
    }
}
