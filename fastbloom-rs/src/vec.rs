use core::mem::size_of;

use crate::builder::SUFFIX;

const USIZE_LEN: usize = 64;
const COUNTER_PER_SLOT: usize = USIZE_LEN >> 2;

/// bitmap only for bloom filter.
#[derive(Debug)]
#[derive(Clone)]
pub(crate) struct BloomBitVec {
    /// Internal representation of the bit vector
    pub(crate) storage: Vec<usize>,
    /// The number of valid bits in the internal representation
    pub(crate) nbits: u64,
}

impl BloomBitVec {
    pub fn new(slots: usize) -> Self {
        BloomBitVec {
            storage: vec![0; slots],
            nbits: (slots * COUNTER_PER_SLOT) as u64,
        }
    }
    pub fn from_elem(slots: usize, bit: bool) -> Self {
        BloomBitVec {
            storage: vec![if bit { !0 } else { 0 }; slots],
            nbits: (slots * COUNTER_PER_SLOT) as u64,
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize) {
        #[cfg(target_pointer_width = "64")]
            let w = index >> 6;
        #[cfg(target_pointer_width = "32")]
            let w = index >> 5;
        let b = index & SUFFIX;
        let flag = 1usize << b;
        self.storage[w] = self.storage[w] | flag;
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        #[cfg(target_pointer_width = "64")]
            let w = index >> 6;
        #[cfg(target_pointer_width = "32")]
            let w = index >> 5;
        let b = index & SUFFIX;
        let flag = 1usize << b;
        (self.storage[w] & flag) != 0
    }

    pub fn or(&mut self, other: &BloomBitVec) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m |= *o;
        }
    }

    pub fn xor(&mut self, other: &BloomBitVec) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m ^= *o;
        }
    }

    pub fn nor(&mut self, other: &Self) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m = !(*m | *o);
        }
    }

    pub fn xnor(&mut self, other: &Self) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m = !(*m ^ *o);
        }
    }

    pub fn and(&mut self, other: &BloomBitVec) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m &= *o;
        }
    }

    pub fn nand(&mut self, other: &Self) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m = !(*m & *o);
        }
    }

    pub fn difference(&mut self, other: &Self) {
        for (m, o) in self.storage.iter_mut().zip(&other.storage) {
            *m &= !*o;
        }
    }


    pub fn clear(&mut self) {
        self.storage.fill(0);
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
}

pub trait Storage {
    type Init;
    fn new(slots: usize, init: Self::Init) -> Self;
    fn get(&self, slot: usize) -> usize;
    fn slots(&self) -> usize;
}
pub trait StorageMut: Storage {
    fn update(&mut self, slot: usize, op: impl FnOnce(usize) -> Option<usize>);
    fn clear(&mut self);
}

impl Storage for Vec<usize> {
    type Init = ();
    #[inline]
    fn new(slots: usize, _: ()) -> Self {
        vec![0; slots]
    }
    #[inline]
    fn get(&self, slot: usize) -> usize {
        self[slot]
    }
    #[inline]
    fn slots(&self) -> usize {
        self.len()
    }
}
impl StorageMut for Vec<usize> {
    #[inline]
    fn update(&mut self, slot: usize, op: impl FnOnce(usize) -> Option<usize>) {
        let v = self[slot];
        if let Some(v) = op(v) {
            self[slot] = v;
        }
    }
    #[inline]
    fn clear(&mut self) {
        self.fill(0);
    }
}

/// counter vector for counting bloom filter.
#[derive(Debug)]
#[derive(Clone)]
pub(crate) struct CountingVec<S> {
    /// Internal representation of the vector
    pub(crate) storage: S,
}
impl<S: Storage> CountingVec<S> {
    /// create a CountingVec
    pub fn new(storage: S) -> Self {
        CountingVec {
            storage,
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> usize {
        let w = index >> 4;
        let b = index & 0b1111;
        let slot = self.storage.get(w);
        (slot >> ((15 - b) * 4)) & 0b1111
    }

    pub fn counters(&self) -> usize {
        self.storage.slots() * COUNTER_PER_SLOT
    }
}
impl<S: StorageMut> CountingVec<S> {
    #[inline]
    pub fn increment(&mut self, index: usize) {
        let w = index >> 4;
        let b = index & 0b1111;
        self.storage.update(w, |slot| {
            let current = (slot >> ((15 - b) * 4)) & 0b1111;
            if current != 0b1111 {
                let current = current + 1;
                let move_bits = (15 - b) * 4;
                Some((slot & !(0b1111 << move_bits)) | (current << move_bits))
            } else {
                None
            }
        });
    }

    #[inline]
    pub fn decrement(&mut self, index: usize) {
        let w = index >> 4;
        let b = index & 0b1111;
        self.storage.update(w, |slot| {
            let current = (slot >> ((15 - b) * 4)) & 0b1111;
            if current > 0 {
                let current = current - 1;
                let w = index >> 4;
                let b = index & 0b1111;
                let move_bits = (15 - b) * 4;
                Some((slot & !(0b1111 << move_bits)) | (current << move_bits))
            } else {
                None
            }
        });
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }
}

#[test]
fn test_vec() {
    let mut vec = BloomBitVec::new(16);
    vec.set(37);
    vec.set(38);
    println!("{:?}", vec);
    assert_eq!(vec.get(37), true);
    assert_eq!(vec.get(38), true);
}

#[test]
fn test_size() {
    println!("{}", COUNTER_PER_SLOT);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(COUNTER_PER_SLOT, 64);
    #[cfg(target_pointer_width = "32")]
    assert_eq!(COUNTER_PER_SLOT, 32);
}

#[test]
fn test_count_vec() {
    let mut vec = CountingVec::new(vec![0; 10]);
    vec.increment(7);

    assert_eq!(1, vec.get(7))
}