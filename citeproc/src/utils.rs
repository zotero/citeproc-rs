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
        let mut result: Vec<T> = Vec::with_capacity(len + (len - 1) * sep.len());
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
