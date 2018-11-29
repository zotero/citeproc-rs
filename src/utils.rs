use cfg_if::cfg_if;

cfg_if! {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    if #[cfg(target_arch = "wasm32")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    }
}

pub trait Intercalate<T> {
    fn intercalate(&self, sep: &T) -> Vec<T>;
}

pub trait JoinMany<T> {
    fn join_many(&self, sep: &[T]) -> Vec<T>;
}

impl<T: Clone> JoinMany<T> for [Vec<T>] {
    fn join_many(&self, sep: &[T]) -> Vec<T> {
        let mut iter = self.iter();
        let first = match iter.next() {
            Some(first) => first,
            None => return vec![],
        };
        let len = self.len();
        let mut result: Vec<T> = Vec::with_capacity(len + (len-1) * sep.len());
        result.extend_from_slice(first);

        for v in iter {
            result.extend_from_slice(&sep);
            result.extend_from_slice(v);
        }
        result
    }
}

impl<T: Clone> Intercalate<T> for [T] {
    fn intercalate(&self, sep: &T) -> Vec<T> {
        let mut iter = self.iter();
        let first = match iter.next() {
            Some(first) => first,
            None => return vec![],
        };
        let mut result: Vec<T> = Vec::with_capacity(self.len() * 2 - 1);
        result.push(first.clone());

        for v in iter {
            result.push(sep.clone());
            result.push(v.clone())
        }
        result
    }
}
