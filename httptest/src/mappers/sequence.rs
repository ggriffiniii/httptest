//! Mappers that handle sequences of items.

use httptest_core::Mapper;

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
