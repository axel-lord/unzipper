//! [Cloner] impl.

use ::core::{convert::identity, num::NonZero};

/// Create a cloner for specified value.
#[expect(unused, reason = "available for future use")]
pub const fn cloner<T>(value: T, size: NonZero<usize>) -> impl DoubleEndedIterator<Item = T>
where
    T: Clone,
{
    Cloner::new(value, size, T::clone, identity)
}

/// Create a fallible cloner for specified value.
pub const fn fallible_cloner<T, F, E>(
    value: T,
    size: NonZero<usize>,
    clone: F,
) -> impl DoubleEndedIterator<Item = Result<T, E>>
where
    F: for<'a> FnMut(&'a T) -> Result<T, E>,
{
    Cloner::new(value, size, clone, Ok)
}

/// Iterator producing values from an item whilst converting it to the last item.
#[derive(Debug, Clone)]
pub struct Cloner<T, P, C> {
    /// Function producing from item.
    produce: P,
    /// Item to clone and how many times to clone it.
    state: Option<(T, C, NonZero<usize>)>,
}

impl<T, P, C> Cloner<T, P, C> {
    /// Construct a new cloner Iterator.
    const fn new(value: T, size: NonZero<usize>, produce: P, convert: C) -> Self {
        Self {
            produce,
            state: Some((value, convert, size)),
        }
    }
}

impl<T, P, C, I> Iterator for Cloner<T, P, C>
where
    P: for<'a> FnMut(&'a T) -> I,
    C: FnOnce(T) -> I,
{
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        let (t, _, i) = self.state.as_mut()?;

        if let Some(next) = NonZero::new(i.get() - 1) {
            *i = next;
            Some((self.produce)(t))
        } else {
            let (t, c, _) = self.state.take()?;
            Some(c(t))
        }
    }
}

impl<T, P, C, I> DoubleEndedIterator for Cloner<T, P, C>
where
    P: for<'a> FnMut(&'a T) -> I,
    C: FnOnce(T) -> I,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.next()
    }
}
