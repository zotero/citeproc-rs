#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

pub mod style;
mod utils;

use self::style::drive_style;

#[macro_use]
extern crate strum_macros;

extern crate cfg_if;
use cfg_if::cfg_if;

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {

        extern crate wasm_bindgen;
        use wasm_bindgen::prelude::*;

        #[wasm_bindgen]
        extern {
            fn alert(s: &str);
        }

        #[wasm_bindgen]
        pub fn greet() {
            alert("Hello, {{project-name}}!");
        }

        #[wasm_bindgen]
        pub fn parse(str: &str) -> String {
            drive_style("in-memory", &str.to_owned())
        }

    }
}
