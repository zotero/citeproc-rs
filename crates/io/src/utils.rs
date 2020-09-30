// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright © 2018 Corporation for Digital Scholarship

use crate::String;

pub fn to_bijective_base_26(int: u32) -> String {
    let mut n = int;
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.insert(0, char::from((65 + 32 + (n % 26)) as u8));
        n /= 26;
    }
    s
}

pub trait JoinMany<T> {
    fn join_many(&self, sep: &[T]) -> Vec<T>;
}

impl<T: Clone> JoinMany<T> for [Vec<T>] {
    fn join_many(&self, sep: &[T]) -> Vec<T> {
        let mut iter = self.iter();
        // TODO: build up the first vec instead of moving it to a new with_capacity one
        let first = match iter.next() {
            Some(first) => first,
            None => return vec![],
        };
        let len = self.len();
        let mut result: Vec<T> = Vec::with_capacity(len + (len - 1) * sep.len());
        result.extend_from_slice(first);

        for v in iter {
            result.extend_from_slice(&sep);
            result.extend_from_slice(v);
        }
        result
    }
}

pub trait Intercalate<T> {
    fn intercalate(self, sep: &T) -> Vec<T>;
}

impl<T, I> Intercalate<T> for I
where
    T: Clone,
    I: Iterator<Item = T>,
{
    fn intercalate(self, sep: &T) -> Vec<T> {
        let mut iter = self;
        let first = match iter.next() {
            Some(first) => first,
            None => return vec![],
        };
        let mut result: Vec<T> = Vec::new();
        result.push(first);

        for v in iter {
            result.push(sep.clone());
            result.push(v.clone())
        }
        result
    }
}

/// Slightly optimised version of Intercalate that can use Vec::with_capacity().
/// When specialization is stable: https://github.com/rust-lang/rust/issues/31844
/// you could reimplement this as a specialization for I: ExactSizeIterator.
pub trait IntercalateExact<T> {
    fn intercalate_exact(self, sep: &T) -> Vec<T>;
}

impl<T, I> IntercalateExact<T> for I
where
    T: Clone,
    I: ExactSizeIterator<Item = T>,
{
    fn intercalate_exact(self, sep: &T) -> Vec<T> {
        let count = self.len();
        let mut iter = self;
        let first = match iter.next() {
            Some(first) => first,
            None => return vec![],
        };
        let mut result: Vec<T> = Vec::with_capacity(count * 2 - 1);
        result.push(first);
        for v in iter {
            result.push(sep.clone());
            result.push(v.clone())
        }
        result
    }
}
