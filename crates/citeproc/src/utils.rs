// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

pub fn to_bijective_base_26(int: u32) -> String {
    let mut n = int;
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.push(char::from((65 + 32 + (n % 26)) as u8));
        n /= 26;
    }
    s
}

use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(feature = "thread")] {
        #[allow(dead_code)]
        pub type Rc<T> = std::sync::Arc<T>;
    } else {
        #[allow(dead_code)]
        pub type Rc<T> = std::rc::Rc<T>;
    }
}
