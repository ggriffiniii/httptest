//! Mappers that handle sequences of items.

use super::Mapper;

/// true if the provided mapper returns true for any of the elements in the
/// sequence.
pub fn contains<C>(inner: C) -> Contains<C> {
    Contains(inner)
}
/// The `Contains` mapper returned by [contains()](fn.contains.html)
#[derive(Debug)]
pub struct Contains<C>(C);
impl<C, E> Mapper<[E]> for Contains<C>
where
    C: Mapper<E, Out = bool>,
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

impl<K, V, KMapper, VMapper> Mapper<(K, V)> for (KMapper, VMapper)
where
    KMapper: Mapper<K, Out = bool>,
    VMapper: Mapper<V, Out = bool>,
{
    type Out = bool;

    fn map(&mut self, input: &(K, V)) -> bool {
        self.0.map(&input.0) && self.1.map(&input.1)
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
        let kv = ("key1", "value1");
        assert_eq!(true, (matches("key1"), any()).map(&kv));
        assert_eq!(true, (matches("key1"), matches("value1")).map(&kv));
        assert_eq!(false, (matches("key1"), matches("value2")).map(&kv));
        assert_eq!(false, (matches("key2"), matches("value1")).map(&kv));
    }
}
