// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

extern crate typed_arena;
use typed_arena::Arena;

fn main() {
    let arena = Arena::new();

    let a = arena.alloc_extend(&['a', 'b', 'c', 'd']);
    {
        let b = arena.alloc_extend(&['K', 'b', 'c', 'd', 'e']);
    }
    println!("{:?}", arena.into_vec());
}
