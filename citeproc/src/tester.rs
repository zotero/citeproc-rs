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
