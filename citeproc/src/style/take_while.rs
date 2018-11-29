// https://stackoverflow.com/questions/28776630/implementing-a-cautious-take-while-using-peekable/28777216#28777216

use std::iter::Peekable;

fn main() {
    let mut chars = "abcdefg.".chars().peekable();

    let abc: String = CautiousTakeWhile{inner: chars.by_ref(), condition: |&x| x != 'd'}.collect();
    let defg: String = CautiousTakeWhile{inner: chars.by_ref(), condition: |&x| x != '.'}.collect();
    println!("{}, {}", abc, defg);
}

pub struct CautiousTakeWhile<'a, I, P>
    where I::Item: 'a,
          I: Iterator + 'a,
          P: FnMut(&I::Item) -> bool,
{
    pub inner: &'a mut Peekable<I>,
    pub condition: P,
}

impl<'a, I, P> Iterator for CautiousTakeWhile<'a, I, P>
    where I::Item: 'a,
          I: Iterator + 'a,
          P: FnMut(&I::Item) -> bool
{
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        let return_next =
            match self.inner.peek() {
                Some(ref v) => (self.condition)(v),
                _ => false,
            };
        if return_next { self.inner.next() } else { None }
    }
}
