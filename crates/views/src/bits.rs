//! A growable bit set that allocates only when a bit is first set, so an
//! instance nobody writes to costs nothing.

#[derive(Default)]
pub(crate) struct Bits(Vec<u64>);

impl Bits {
    /// Set bit `i`; returns whether it was already set.
    pub(crate) fn set(&mut self, i: usize) -> bool {
        let (word, mask) = (i / 64, 1u64 << (i % 64));
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        let was = self.0[word] & mask != 0;
        self.0[word] |= mask;
        was
    }

    pub(crate) fn clear(&mut self, i: usize) {
        if let Some(word) = self.0.get_mut(i / 64) {
            *word &= !(1u64 << (i % 64));
        }
    }
}
