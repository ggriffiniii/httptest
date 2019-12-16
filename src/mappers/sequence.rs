//! Mappers that handle sequences of items.

use super::Mapper;

use std::borrow::{Borrow, ToOwned};

/// A key-value pair.
#[derive(Debug, PartialEq, PartialOrd)]
pub struct KV<K, V>
where
    Self: Sized,
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
{
    /// The key
    pub k: K::Owned,
    /// The value
    pub v: V::Owned,
}

/// true if the provided mapper returns true for any of the elements in the
/// sequence.
pub fn contains<M>(inner: M) -> Contains<M> {
    Contains(inner)
}
/// The `Contains` mapper returned by [contains()](fn.contains.html)
#[derive(Debug)]
pub struct Contains<M>(M);
impl<M, E> Mapper<[E]> for Contains<M>
where
    M: Mapper<E, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &[E]) -> bool {
        for elem in input {
            if self.0.map(elem) {
                return true;
            }
        }
        false
    }
}

impl<K, V, KMapper, VMapper> Mapper<KV<K, V>> for (KMapper, VMapper)
where
    K: ToOwned + ?Sized,
    V: ToOwned + ?Sized,
    KMapper: Mapper<K, Out = bool>,
    VMapper: Mapper<V, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &KV<K, V>) -> bool {
        self.0.map(input.k.borrow()) && self.1.map(input.v.borrow())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mappers::*;

    #[test]
    fn test_contains() {
        let mut c = contains(eq(100));
        assert_eq!(true, c.map(vec![100, 200, 300].as_slice()));
        assert_eq!(false, c.map(vec![99, 200, 300].as_slice()));
    }

    #[test]
    fn test_tuple() {
        let kv: KV<str, str> = KV {
            k: "key1".to_owned(),
            v: "value1".to_owned(),
        };
        assert_eq!(true, (matches("key1"), any()).map(&kv));
        assert_eq!(true, (matches("key1"), matches("value1")).map(&kv));
        assert_eq!(false, (matches("key1"), matches("value2")).map(&kv));
        assert_eq!(false, (matches("key2"), matches("value1")).map(&kv));
    }
}
