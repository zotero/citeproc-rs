/// Wraps a function that should be available from C in no_mangle/extern, and a panic::catch_unwind
/// block so panics cannot unwind outside the Rust stack into C territory. Any caught panics will
/// abort the process.
///
/// Taken from hyperium/hyper src/ffi/macros.rs
macro_rules! ffi_fn {
    ($(#[$doc:meta])* fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty $body:block) => {
        $(#[$doc])*
        #[no_mangle]
        pub extern fn $name($($arg: $arg_ty),*) -> $ret {
            use std::panic::{self, AssertUnwindSafe};

            // The blanket AssertUnwindSafe is only possible because if we ever do panic we abort,
            // so you can never use a returned value again anyway.
            // If you remove the abort, you have to remove the AssertUnwindSafe.
            match panic::catch_unwind(AssertUnwindSafe(move || $body)) {
                Ok(v) => v,
                Err(e) => {
                    let err = $crate::FFIError::from_caught_panic(e);
                    log::error!("{}", err);
                    return $crate::nullable::FromErrorCode::from_error_code(
                        $crate::errors::update_last_error_return_code(err)
                    );
                }
            }
        }
    };

    ($(#[$doc:meta])* fn $name:ident($($arg:ident: $arg_ty:ty),*) $body:block) => {
        ffi_fn!($(#[$doc])* fn $name($($arg: $arg_ty),*) -> () $body);
    };
}

macro_rules! ffi_fn_nullify {
    ($(#[$doc:meta])* $(@safety $unsafe:tt)? fn $name:ident($( $(#[$wrapper:ident])? $arg:ident: $arg_ty:ty),*) -> $ret:ty $body:block) => {
        $(#[$doc])*
        #[no_mangle]
        pub $($unsafe)? extern fn $name($( $arg: $arg_ty),*) -> $ret {
            use std::panic;
            use $crate::macros::MakeUnwindSafe;
            $(
                // funky: if a wrapper isn't provided, we just get (T,). MakeUnwindSafe is
                // implemented on (T,) for all T: Sized, as a no-op.
                let mut $arg = $($wrapper)?($arg,);
            )*

            // The blanket AssertUnwindSafe is only possible because if we ever do panic we abort,
            // so you can never use a returned value again anyway.
            // If you remove the abort, you have to remove the AssertUnwindSafe.
            match panic::catch_unwind(#[allow(unused_mut)] move || {
                $(let mut $arg = $arg.0;)*
                $body
            }) {
                Ok(v) => v,
                Err(e) => {
                    $($arg.make_unwind_safe();)*
                    let err = $crate::FFIError::from_caught_panic(e);
                    log::error!("{}", err);
                    return $crate::nullable::FromErrorCode::from_error_code(
                        $crate::errors::update_last_error_return_code(err)
                    );
                }
            }
        }
    };

    ($(#[$doc:meta])* fn $name:ident($($arg:ident: $arg_ty:ty),*) $body:block) => {
        ffi_fn_nullify!($(#[$doc])* fn $name($($arg: $arg_ty),*) -> () $body);
    };
}

/// used in #[nullify_on_panic] attributes in the [ffi_fn_nullify] macro.
pub fn nullify_on_panic<T: MakeUnwindSafe>(ptr: *mut T) -> PointerSetNull<T> {
    PointerSetNull(ptr)
}

pub trait MakeUnwindSafe: Sized {
    fn make_unwind_safe(&mut self);
}

/// A struct to hold a *mut T and implement MakeUnwindSafe.
pub struct PointerSetNull<T>(pub *mut T);
impl<T> Clone for PointerSetNull<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T> Copy for PointerSetNull<T> {}
impl<T> MakeUnwindSafe for PointerSetNull<T>
where
    T: Sized + MakeUnwindSafe,
{
    fn make_unwind_safe(&mut self) {
        let ptr = self.0;
        if ptr.is_null() {
            return;
        }
        // just checked for null
        let t = unsafe { &mut *ptr };
        // doesn't set the pointer to null, sets T to its null value. That will usually be Option::None.
        t.make_unwind_safe()
    }
}

/// See $wrapper above; when no $wrapper function provided, (T,) is what you get.
/// No-op implementation.
impl<T> MakeUnwindSafe for (T,)
where
    T: Sized,
{
    fn make_unwind_safe(&mut self) {}
}
