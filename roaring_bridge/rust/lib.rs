//! Crubit-friendly wrapper around the `roaring` crate.
//!
//! This crate provides `RoaringBitmap32` (32-bit) and `RoaringBitmap64` (64-bit)
//! roaring bitmap types with explicit, non-generic method signatures that Crubit
//! can automatically expose to C++.
//!
//! The C++ namespace is `roaring_bridge`.

use roaring::{MultiOps, RoaringBitmap, RoaringTreemap};

#[allow(non_camel_case_types)]
type vector<T> = Vec<T>;

/// A compressed 32-bit roaring bitmap.
///
/// This is a Crubit-friendly wrapper around [`roaring::RoaringBitmap`].
/// Equivalent to CRoaring's `roaring::Roaring`.
#[derive(Clone, Default)]
pub struct RoaringBitmap32 {
    inner: RoaringBitmap,
}

// Since this is used to generate C++ we use the C++ style guide for naming.
// See https://google.github.io/styleguide/cppguide.html
#[allow(non_snake_case)]
impl RoaringBitmap32 {
    /// Creates a full bitmap containing all u32 values.
    pub fn Full() -> Self {
        Self { inner: RoaringBitmap::full() }
    }

    /// Adds a value. Returns true if the value was absent.
    pub fn Insert(&mut self, value: u32) -> bool {
        self.inner.insert(value)
    }

    /// Adds all values from the slice.
    pub fn InsertMany(&mut self, values: &[u32]) {
        self.inner.extend(values.iter());
    }

    /// Adds all values in the half-open range [min, max).
    /// Returns the number of values inserted.
    pub fn InsertRange(&mut self, min: u32, max: u32) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.insert_range(min..max)
    }

    /// Adds all values in the closed range [min, max].
    /// Returns the number of values inserted.
    pub fn InsertRangeClosed(&mut self, min: u32, max: u32) -> u64 {
        self.inner.insert_range(min..=max)
    }

    /// Removes a value. Returns true if the value was present.
    pub fn Remove(&mut self, value: u32) -> bool {
        self.inner.remove(value)
    }

    /// Removes all values in the half-open range [min, max).
    /// Returns the number of values removed.
    pub fn RemoveRange(&mut self, min: u32, max: u32) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.remove_range(min..max)
    }

    /// Removes all values in the closed range [min, max].
    /// Returns the number of values removed.
    pub fn RemoveRangeClosed(&mut self, min: u32, max: u32) -> u64 {
        self.inner.remove_range(min..=max)
    }

    /// Returns true if the value is in the bitmap.
    pub fn Contains(&self, value: u32) -> bool {
        self.inner.contains(value)
    }

    /// Returns true if all values in [min, max) are present.
    pub fn ContainsRange(&self, min: u32, max: u32) -> bool {
        if min >= max {
            return true;
        }
        self.inner.contains_range(min..max)
    }

    /// Returns the number of elements.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Returns true if empty.
    pub fn IsEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns true if the bitmap contains every possible u32 value.
    pub fn IsFull(&self) -> bool {
        self.inner.is_full()
    }

    /// Returns the minimum value, or None if empty.
    pub fn Min(&self) -> Option<u32> {
        self.inner.min()
    }

    /// Returns the maximum value, or None if empty.
    pub fn Max(&self) -> Option<u32> {
        self.inner.max()
    }

    /// Returns the number of values <= x.
    pub fn Rank(&self, x: u32) -> u64 {
        self.inner.rank(x)
    }

    /// Returns the n-th smallest value (0-indexed), or None.
    pub fn Select(&self, n: u32) -> Option<u32> {
        self.inner.select(n)
    }

    /// Returns the number of elements in the range [min, max).
    pub fn RangeCardinality(&self, min: u32, max: u32) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.range_cardinality(min..max)
    }

    /// Clears all values.
    pub fn Clear(&mut self) {
        self.inner.clear();
    }

    /// Optimizes the internal storage for space.
    /// Returns true if the storage was modified.
    pub fn Optimize(&mut self) -> bool {
        self.inner.optimize()
    }

    /// Removes run-length encoding even when it is more space efficient.
    /// Returns true if the storage was modified.
    pub fn RemoveRunCompression(&mut self) -> bool {
        self.inner.remove_run_compression()
    }
    /// Returns the union of all bitmaps in `bitmaps`.
    ///
    /// Returns an empty bitmap if `bitmaps` is empty.
    pub fn Union(bitmaps: &[RoaringBitmap32]) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: bitmaps.iter().map(|b| &b.inner).union() }
    }

    /// Returns the intersection of all bitmaps in `bitmaps`.
    ///
    /// Returns an empty bitmap if `bitmaps` is empty.
    pub fn Intersect(bitmaps: &[RoaringBitmap32]) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: bitmaps.iter().map(|b| &b.inner).intersection() }
    }

    /// Size of intersection without creating a new bitmap.
    pub fn IntersectionLen(&self, other: &RoaringBitmap32) -> u64 {
        self.inner.intersection_len(&other.inner)
    }

    /// Size of union without creating a new bitmap.
    pub fn UnionLen(&self, other: &RoaringBitmap32) -> u64 {
        self.inner.union_len(&other.inner)
    }

    /// Size of difference without creating a new bitmap.
    pub fn DifferenceLen(&self, other: &RoaringBitmap32) -> u64 {
        self.inner.difference_len(&other.inner)
    }

    /// Size of symmetric difference without creating a new bitmap.
    pub fn SymmetricDifferenceLen(&self, other: &RoaringBitmap32) -> u64 {
        self.inner.symmetric_difference_len(&other.inner)
    }

    /// Returns true if self is a subset of other.
    pub fn IsSubset(&self, other: &RoaringBitmap32) -> bool {
        self.inner.is_subset(&other.inner)
    }

    /// Returns a copy of the bitmap with all values shifted by `offset`.
    ///
    /// Values that fall outside the range [0, 2^32) are dropped.
    pub fn AddOffset(&self, offset: i64) -> Self {
        Self { inner: add_offset_32(&self.inner, offset) }
    }

    /// Collects all values into a vector.
    pub fn ToVec(&self) -> vector<u32> {
        self.inner.iter().collect()
    }

    /// Returns the serialized size in bytes.
    pub fn serialized_size(&self) -> usize {
        self.inner.serialized_size()
    }

    /// Serializes to bytes (standard Roaring format, compatible with CRoaring).
    /// Returns an empty vector if serialization fails.
    pub fn Serialize(&self) -> vector<u8> {
        let mut buf = Vec::with_capacity(self.inner.serialized_size());
        if self.inner.serialize_into(&mut buf).is_err() {
            buf.clear();
        }
        buf.into()
    }

    /// Deserializes from bytes (standard Roaring format).
    /// Returns None if the data is invalid.
    pub fn Deserialize(data: &[u8]) -> Option<RoaringBitmap32> {
        RoaringBitmap::deserialize_from(data).ok().map(|inner| RoaringBitmap32 { inner })
    }

    /// Deserializes without validation (faster, for trusted data).
    /// Returns None on I/O error.
    pub fn DeserializeUnchecked(data: &[u8]) -> Option<RoaringBitmap32> {
        RoaringBitmap::deserialize_unchecked_from(data).ok().map(|inner| RoaringBitmap32 { inner })
    }
}

impl PartialEq for RoaringBitmap32 {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl core::ops::BitAndAssign<&RoaringBitmap32> for RoaringBitmap32 {
    fn bitand_assign(&mut self, rhs: &RoaringBitmap32) {
        self.inner &= &rhs.inner;
    }
}

impl core::ops::BitOrAssign<&RoaringBitmap32> for RoaringBitmap32 {
    fn bitor_assign(&mut self, rhs: &RoaringBitmap32) {
        self.inner |= &rhs.inner;
    }
}

impl core::ops::BitXorAssign<&RoaringBitmap32> for RoaringBitmap32 {
    fn bitxor_assign(&mut self, rhs: &RoaringBitmap32) {
        self.inner ^= &rhs.inner;
    }
}

impl core::ops::SubAssign<&RoaringBitmap32> for RoaringBitmap32 {
    fn sub_assign(&mut self, rhs: &RoaringBitmap32) {
        self.inner -= &rhs.inner;
    }
}

impl core::ops::BitAnd<&RoaringBitmap32> for &RoaringBitmap32 {
    type Output = RoaringBitmap32;
    fn bitand(self, rhs: &RoaringBitmap32) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: &self.inner & &rhs.inner }
    }
}

impl core::ops::BitOr<&RoaringBitmap32> for &RoaringBitmap32 {
    type Output = RoaringBitmap32;
    fn bitor(self, rhs: &RoaringBitmap32) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: &self.inner | &rhs.inner }
    }
}

impl core::ops::BitXor<&RoaringBitmap32> for &RoaringBitmap32 {
    type Output = RoaringBitmap32;
    fn bitxor(self, rhs: &RoaringBitmap32) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: &self.inner ^ &rhs.inner }
    }
}

impl core::ops::Sub<&RoaringBitmap32> for &RoaringBitmap32 {
    type Output = RoaringBitmap32;
    fn sub(self, rhs: &RoaringBitmap32) -> RoaringBitmap32 {
        RoaringBitmap32 { inner: &self.inner - &rhs.inner }
    }
}

/// A borrowing iterator over the values in a [`RoaringBitmap32`].
///
/// Yields `u32` values in ascending order without consuming the bitmap.
/// Crubit exposes this as a C++ iterator via `begin()`/`end()` on
/// `&RoaringBitmap32`.
#[derive(Clone)]
pub struct RoaringBitmap32Iter<'a> {
    inner: roaring::bitmap::Iter<'a>,
}

impl Iterator for RoaringBitmap32Iter<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> IntoIterator for &'a RoaringBitmap32 {
    type Item = u32;
    type IntoIter = RoaringBitmap32Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        RoaringBitmap32Iter { inner: self.inner.iter() }
    }
}

/// A compressed 64-bit roaring bitmap.
///
/// This is a Crubit-friendly wrapper around [`roaring::RoaringTreemap`].
/// Equivalent to CRoaring's `roaring::Roaring64Map`.
#[derive(Clone, Default)]
pub struct RoaringBitmap64 {
    inner: RoaringTreemap,
}

#[allow(non_snake_case)]
impl RoaringBitmap64 {
    /// Adds a value. Returns true if the value was absent.
    pub fn Insert(&mut self, value: u64) -> bool {
        self.inner.insert(value)
    }

    /// Adds all values from the slice.
    pub fn InsertMany(&mut self, values: &[u64]) {
        self.inner.extend(values.iter());
    }

    /// Adds all values in the half-open range [min, max).
    /// Returns the number of values inserted.
    pub fn InsertRange(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.insert_range(min..max)
    }

    /// Adds all values in the closed range [min, max].
    /// Returns the number of values inserted.
    pub fn InsertRangeClosed(&mut self, min: u64, max: u64) -> u64 {
        self.inner.insert_range(min..=max)
    }

    /// Removes a value. Returns true if the value was present.
    pub fn Remove(&mut self, value: u64) -> bool {
        self.inner.remove(value)
    }

    /// Removes all values in the half-open range [min, max).
    /// Returns the number of values removed.
    pub fn RemoveRange(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.remove_range(min..max)
    }

    /// Removes all values in the closed range [min, max].
    /// Returns the number of values removed.
    pub fn RemoveRangeClosed(&mut self, min: u64, max: u64) -> u64 {
        self.inner.remove_range(min..=max)
    }

    /// Returns true if the value is in the bitmap.
    pub fn Contains(&self, value: u64) -> bool {
        self.inner.contains(value)
    }

    /// Returns true if all values in [min, max) are present.
    pub fn ContainsRange(&self, min: u64, max: u64) -> bool {
        if min >= max {
            return true;
        }
        self.inner.contains_range(min..max)
    }

    /// Returns the number of elements.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Returns true if empty.
    pub fn IsEmpty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the minimum value, or None if empty.
    pub fn Min(&self) -> Option<u64> {
        self.inner.min()
    }

    /// Returns the maximum value, or None if empty.
    pub fn Max(&self) -> Option<u64> {
        self.inner.max()
    }

    /// Returns the number of values <= x.
    pub fn Rank(&self, x: u64) -> u64 {
        self.inner.rank(x)
    }

    /// Returns the n-th smallest value (0-indexed), or None.
    pub fn Select(&self, n: u64) -> Option<u64> {
        self.inner.select(n)
    }

    /// Returns the number of elements in the range [min, max).
    pub fn RangeCardinality(&self, min: u64, max: u64) -> u64 {
        if min >= max {
            return 0;
        }
        self.inner.range_cardinality(min..max)
    }

    /// Clears all values.
    pub fn Clear(&mut self) {
        self.inner.clear();
    }

    /// Optimizes the internal storage for space.
    /// Returns true if the storage was modified.
    pub fn Optimize(&mut self) -> bool {
        self.inner.optimize()
    }

    /// Returns the union of all bitmaps in `bitmaps`.
    ///
    /// Returns an empty bitmap if `bitmaps` is empty.
    pub fn Union(bitmaps: &[RoaringBitmap64]) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: bitmaps.iter().map(|b| &b.inner).union() }
    }

    /// Returns the intersection of all bitmaps in `bitmaps`.
    ///
    /// Returns an empty bitmap if `bitmaps` is empty.
    pub fn Intersect(bitmaps: &[RoaringBitmap64]) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: bitmaps.iter().map(|b| &b.inner).intersection() }
    }

    /// Size of intersection without creating a new bitmap.
    pub fn IntersectionLen(&self, other: &RoaringBitmap64) -> u64 {
        self.inner.intersection_len(&other.inner)
    }

    /// Size of union without creating a new bitmap.
    pub fn UnionLen(&self, other: &RoaringBitmap64) -> u64 {
        self.inner.union_len(&other.inner)
    }

    /// Size of difference without creating a new bitmap.
    pub fn DifferenceLen(&self, other: &RoaringBitmap64) -> u64 {
        self.inner.difference_len(&other.inner)
    }

    /// Size of symmetric difference without creating a new bitmap.
    pub fn SymmetricDifferenceLen(&self, other: &RoaringBitmap64) -> u64 {
        self.inner.symmetric_difference_len(&other.inner)
    }

    /// Returns true if self is a subset of other.
    pub fn IsSubset(&self, other: &RoaringBitmap64) -> bool {
        self.inner.is_subset(&other.inner)
    }

    /// Returns a copy of the bitmap with all values shifted by `offset`.
    ///
    /// Values that overflow or underflow `u64` are dropped.
    pub fn AddOffset(&self, offset: i64) -> Self {
        Self { inner: add_offset_64(&self.inner, offset) }
    }

    /// Collects all values into a vector.
    pub fn ToVec(&self) -> vector<u64> {
        self.inner.iter().collect()
    }

    /// Returns the serialized size in bytes.
    pub fn serialized_size(&self) -> usize {
        self.inner.serialized_size()
    }

    /// Serializes to bytes.
    /// The format is compatible with CRoaring's Roaring64Map serialization.
    /// Returns an empty vector if serialization fails.
    pub fn Serialize(&self) -> vector<u8> {
        let mut buf = Vec::with_capacity(self.inner.serialized_size());
        if self.inner.serialize_into(&mut buf).is_err() {
            buf.clear();
        }
        buf.into()
    }

    /// Deserializes from bytes.
    /// Returns None if the data is invalid.
    pub fn Deserialize(data: &[u8]) -> Option<RoaringBitmap64> {
        RoaringTreemap::deserialize_from(data).ok().map(|inner| RoaringBitmap64 { inner })
    }

    /// Deserializes without validation (faster, for trusted data).
    /// Returns None on I/O error.
    pub fn DeserializeUnchecked(data: &[u8]) -> Option<RoaringBitmap64> {
        RoaringTreemap::deserialize_unchecked_from(data).ok().map(|inner| RoaringBitmap64 { inner })
    }
}

impl PartialEq for RoaringBitmap64 {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl core::ops::BitAndAssign<&RoaringBitmap64> for RoaringBitmap64 {
    fn bitand_assign(&mut self, rhs: &RoaringBitmap64) {
        self.inner &= &rhs.inner;
    }
}

impl core::ops::BitOrAssign<&RoaringBitmap64> for RoaringBitmap64 {
    fn bitor_assign(&mut self, rhs: &RoaringBitmap64) {
        self.inner |= &rhs.inner;
    }
}

impl core::ops::BitXorAssign<&RoaringBitmap64> for RoaringBitmap64 {
    fn bitxor_assign(&mut self, rhs: &RoaringBitmap64) {
        self.inner ^= &rhs.inner;
    }
}

impl core::ops::SubAssign<&RoaringBitmap64> for RoaringBitmap64 {
    fn sub_assign(&mut self, rhs: &RoaringBitmap64) {
        self.inner -= &rhs.inner;
    }
}

impl core::ops::BitAnd<&RoaringBitmap64> for &RoaringBitmap64 {
    type Output = RoaringBitmap64;
    fn bitand(self, rhs: &RoaringBitmap64) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: &self.inner & &rhs.inner }
    }
}

impl core::ops::BitOr<&RoaringBitmap64> for &RoaringBitmap64 {
    type Output = RoaringBitmap64;
    fn bitor(self, rhs: &RoaringBitmap64) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: &self.inner | &rhs.inner }
    }
}

impl core::ops::BitXor<&RoaringBitmap64> for &RoaringBitmap64 {
    type Output = RoaringBitmap64;
    fn bitxor(self, rhs: &RoaringBitmap64) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: &self.inner ^ &rhs.inner }
    }
}

impl core::ops::Sub<&RoaringBitmap64> for &RoaringBitmap64 {
    type Output = RoaringBitmap64;
    fn sub(self, rhs: &RoaringBitmap64) -> RoaringBitmap64 {
        RoaringBitmap64 { inner: &self.inner - &rhs.inner }
    }
}

/// A borrowing iterator over the values in a [`RoaringBitmap64`].
///
/// Yields `u64` values in ascending order without consuming the bitmap.
/// Crubit exposes this as a C++ iterator via `begin()`/`end()` on
/// `&RoaringBitmap64`.
#[derive(Clone)]
pub struct RoaringBitmap64Iter<'a> {
    inner: roaring::treemap::Iter<'a>,
}

impl Iterator for RoaringBitmap64Iter<'_> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> IntoIterator for &'a RoaringBitmap64 {
    type Item = u64;
    type IntoIter = RoaringBitmap64Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        RoaringBitmap64Iter { inner: self.inner.iter() }
    }
}

const SERIAL_COOKIE_NO_RUNCONTAINER: u32 = 12346;
const NO_OFFSET_THRESHOLD: usize = 4;
const ARRAY_LIMIT: usize = 4096;
const BITMAP_LENGTH: usize = 1024;
const BITMAP_BYTES: usize = 8192;

enum PendingStore {
    Array(Vec<u16>),
    Bitmap { card: u64, words: Box<[u64; BITMAP_LENGTH]> },
}

struct PendingContainer {
    key: u16,
    store: PendingStore,
}

impl PendingContainer {
    fn new_array(key: u16, cap: usize) -> Self {
        Self { key, store: PendingStore::Array(Vec::with_capacity(cap)) }
    }

    fn new_bitmap(key: u16) -> Self {
        Self {
            key,
            store: PendingStore::Bitmap { card: 0, words: Box::new([0u64; BITMAP_LENGTH]) },
        }
    }

    fn ensure_bitmap(&mut self) -> (&mut u64, &mut [u64; BITMAP_LENGTH]) {
        if let PendingStore::Array(vec) = &self.store {
            let mut words = Box::new([0u64; BITMAP_LENGTH]);
            let card = vec.len() as u64;
            for &v in vec {
                words[(v >> 6) as usize] |= 1u64 << (v & 63);
            }
            self.store = PendingStore::Bitmap { card, words };
        }
        match &mut self.store {
            PendingStore::Bitmap { card, words } => (card, words),
            PendingStore::Array(_) => unreachable!(),
        }
    }

    fn insert_disjoint_range(&mut self, start: u16, end: u16) {
        let len = (end as usize) - (start as usize) + 1;
        if let PendingStore::Array(vec) = &mut self.store {
            if vec.len() + len <= ARRAY_LIMIT {
                vec.extend(start..=end);
                return;
            }
        }
        let (card, words) = self.ensure_bitmap();
        let start_word = (start >> 6) as usize;
        let end_word = (end >> 6) as usize;
        let start_bit = (start & 63) as u32;
        let end_bit = (end & 63) as u32;
        if start_word == end_word {
            let mask = (!0u64 << start_bit) & (!0u64 >> (63 - end_bit));
            words[start_word] |= mask;
        } else {
            words[start_word] |= !0u64 << start_bit;
            for w in &mut words[(start_word + 1)..end_word] {
                *w = !0u64;
            }
            words[end_word] |= !0u64 >> (63 - end_bit);
        }
        *card += len as u64;
    }
}

fn flush_pending(
    pending: &mut Option<PendingContainer>,
    descriptions: &mut Vec<(u16, u16)>,
    payload_buf: &mut Vec<u8>,
) {
    let Some(p) = pending.take() else {
        return;
    };
    match p.store {
        PendingStore::Array(vec) => {
            if vec.is_empty() {
                return;
            }
            descriptions.push((p.key, (vec.len() - 1) as u16));
            payload_buf.reserve(vec.len() * 2);
            for &v in &vec {
                payload_buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        PendingStore::Bitmap { card, words } => {
            if card == 0 {
                return;
            }
            descriptions.push((p.key, (card - 1) as u16));
            if (card as usize) <= ARRAY_LIMIT {
                payload_buf.reserve((card as usize) * 2);
                for (word_idx, &w) in words.iter().enumerate() {
                    let mut word = w;
                    let base = (word_idx as u16) << 6;
                    while word != 0 {
                        let bit = word.trailing_zeros() as u16;
                        payload_buf.extend_from_slice(&(base + bit).to_le_bytes());
                        word &= word - 1;
                    }
                }
            } else {
                let mut raw = [0u8; BITMAP_BYTES];
                for (dst, &w) in raw.chunks_exact_mut(8).zip(words.iter()) {
                    dst.copy_from_slice(&w.to_le_bytes());
                }
                payload_buf.extend_from_slice(&raw);
            }
        }
    }
}

#[inline(always)]
fn read_u16_le(bytes: &[u8], idx: usize) -> u16 {
    let off = idx * 2;
    u16::from_le_bytes([bytes[off], bytes[off + 1]])
}

#[inline(always)]
fn read_u64_le(bytes: &[u8; BITMAP_BYTES], word_idx: usize) -> u64 {
    let off = word_idx * 8;
    u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
}

/// Element-by-element reference implementation of `add_offset_32`.
///
/// Requires `offset` to be in `[-(u32::MAX), u32::MAX]`. Used directly for small bitmaps, where
/// the container-level machinery is not worth its setup cost, and as a fallback should the
/// container-level path ever produce bytes the `roaring` crate refuses to deserialize.
fn add_offset_32_naive(bitmap: &RoaringBitmap, offset: i64) -> RoaringBitmap {
    if offset == 0 {
        return bitmap.clone();
    }
    if offset > 0 {
        let off = offset as u32;
        RoaringBitmap::from_sorted_iter(bitmap.range(..=(u32::MAX - off)).map(|x| x + off))
            .expect("iterator is sorted")
    } else {
        let neg_off = offset.unsigned_abs() as u32;
        RoaringBitmap::from_sorted_iter(bitmap.range(neg_off..).map(|x| x - neg_off))
            .expect("iterator is sorted")
    }
}

/// Shifts every value in `bitmap` by `offset`, dropping values that leave `[0, 2^32)`.
///
/// The `roaring` crate does not expose its containers, so the only way to shift at container
/// granularity (rather than inserting every element one at a time) is to round-trip through the
/// portable serialization format: serialize, rewrite the container descriptors and payloads in
/// the byte buffer, and deserialize the result. A shift by `offset` decomposes into a shift of the
/// 16-bit container keys by `offset >> 16` plus a shift of the values inside each container by
/// `offset & 0xFFFF`; the latter splits each source container across two adjacent destination
/// containers, which is what the bulk of this function handles.
fn add_offset_32(bitmap: &RoaringBitmap, offset: i64) -> RoaringBitmap {
    if offset == 0 {
        return bitmap.clone();
    }
    if offset > u32::MAX as i64 || offset < -(u32::MAX as i64) {
        return RoaringBitmap::new();
    }
    let (Some(min_val), Some(max_val)) = (bitmap.min(), bitmap.max()) else {
        return RoaringBitmap::new();
    };
    if (max_val as i64) + offset < 0 || (min_val as i64) + offset > u32::MAX as i64 {
        return RoaringBitmap::new();
    }
    if bitmap.len() <= 256 {
        return add_offset_32_naive(bitmap, offset);
    }

    let mut buf = Vec::with_capacity(bitmap.serialized_size());
    if bitmap.serialize_into(&mut buf).is_err() {
        return add_offset_32_naive(bitmap, offset);
    }

    let cookie = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let (size, has_run, desc_start) = if cookie == SERIAL_COOKIE_NO_RUNCONTAINER {
        let size = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
        (size, false, 8usize)
    } else {
        let size = ((cookie >> 16) + 1) as usize;
        let run_bytes = size.div_ceil(8);
        (size, true, 4 + run_bytes)
    };

    let container_offset = offset >> 16;
    let in_offset = (offset & 0xFFFF) as u16;

    // Fast path for container-aligned offsets where all containers stay in
    // [0, 65536): adjust the 16-bit container keys in-place and deserialize.
    if in_offset == 0
        && (min_val as i64) + offset >= 0
        && (max_val as i64) + offset <= u32::MAX as i64
    {
        for i in 0..size {
            let pos = desc_start + i * 4;
            let key = u16::from_le_bytes([buf[pos], buf[pos + 1]]);
            let new_key = (key as i64 + container_offset) as u16;
            buf[pos..pos + 2].copy_from_slice(&new_key.to_le_bytes());
        }
        return RoaringBitmap::deserialize_unchecked_from(&buf[..])
            .unwrap_or_else(|_| add_offset_32_naive(bitmap, offset));
    }

    let has_offsets = !has_run || size >= NO_OFFSET_THRESHOLD;
    let mut cursor = desc_start + size * 4 + if has_offsets { size * 4 } else { 0 };

    let mut descriptions: Vec<(u16, u16)> = Vec::with_capacity(size + 1);
    let mut payload_buf: Vec<u8> = Vec::with_capacity(buf.len());
    let mut pending: Option<PendingContainer> = None;

    for i in 0..size {
        let desc_pos = desc_start + i * 4;
        let orig_key = u16::from_le_bytes([buf[desc_pos], buf[desc_pos + 1]]);
        let card_minus_1 = u16::from_le_bytes([buf[desc_pos + 2], buf[desc_pos + 3]]);
        let cardinality = (card_minus_1 as usize) + 1;
        let is_run = has_run && (buf[4 + i / 8] & (1 << (i % 8)) != 0);

        let payload_len = if is_run {
            let runs = u16::from_le_bytes([buf[cursor], buf[cursor + 1]]) as usize;
            2 + runs * 4
        } else if cardinality <= ARRAY_LIMIT {
            cardinality * 2
        } else {
            BITMAP_BYTES
        };

        let k = (orig_key as i64) + container_offset;

        if in_offset == 0 {
            if (0..=0xFFFF).contains(&k) {
                if !is_run {
                    descriptions.push((k as u16, card_minus_1));
                    payload_buf.extend_from_slice(&buf[cursor..cursor + payload_len]);
                } else {
                    let runs = u16::from_le_bytes([buf[cursor], buf[cursor + 1]]) as usize;
                    let run_bytes = &buf[cursor + 2..cursor + 2 + runs * 4];
                    let mut p = PendingContainer::new_array(k as u16, cardinality.min(ARRAY_LIMIT));
                    for chunk in run_bytes.chunks_exact(4) {
                        let start = u16::from_le_bytes([chunk[0], chunk[1]]);
                        let len_m1 = u16::from_le_bytes([chunk[2], chunk[3]]);
                        p.insert_disjoint_range(start, start + len_m1);
                    }
                    let mut opt_p = Some(p);
                    flush_pending(&mut opt_p, &mut descriptions, &mut payload_buf);
                }
            }
            cursor += payload_len;
            continue;
        }

        let k_hi = k + 1;
        let need_lo = (0..=0xFFFF).contains(&k);
        let need_hi = (0..=0xFFFF).contains(&k_hi);

        if !need_lo && !need_hi {
            cursor += payload_len;
            continue;
        }

        if is_run {
            let runs = u16::from_le_bytes([buf[cursor], buf[cursor + 1]]) as usize;
            let run_bytes = &buf[cursor + 2..cursor + 2 + runs * 4];
            if need_lo {
                for chunk in run_bytes.chunks_exact(4) {
                    let start = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
                    let len_m1 = u16::from_le_bytes([chunk[2], chunk[3]]) as u32;
                    let s = start + (in_offset as u32);
                    if s < 65536 {
                        let e = (s + len_m1).min(65535);
                        if pending.as_ref().is_some_and(|p| p.key != k as u16) {
                            flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                        }
                        let p = pending.get_or_insert_with(|| {
                            PendingContainer::new_array(
                                k as u16,
                                ((e - s + 1) as usize).min(ARRAY_LIMIT),
                            )
                        });
                        p.insert_disjoint_range(s as u16, e as u16);
                    }
                }
            }
            if need_hi {
                flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                for chunk in run_bytes.chunks_exact(4) {
                    let start = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
                    let len_m1 = u16::from_le_bytes([chunk[2], chunk[3]]) as u32;
                    let s = start + (in_offset as u32);
                    let e = s + len_m1;
                    if e >= 65536 {
                        let hi_s = (s.max(65536) - 65536) as u16;
                        let hi_e = (e - 65536) as u16;
                        let p = pending.get_or_insert_with(|| {
                            PendingContainer::new_array(
                                k_hi as u16,
                                ((hi_e - hi_s + 1) as usize).min(ARRAY_LIMIT),
                            )
                        });
                        p.insert_disjoint_range(hi_s, hi_e);
                    }
                }
            }
        } else if cardinality <= ARRAY_LIMIT {
            let raw_bytes = &buf[cursor..cursor + payload_len];
            let split_val = 0u16.wrapping_sub(in_offset);
            let mut lo = 0usize;
            let mut hi = cardinality;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if read_u16_le(raw_bytes, mid) < split_val {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            let split_idx = lo;

            if need_lo && split_idx > 0 {
                if pending.as_ref().is_some_and(|p| p.key != k as u16) {
                    flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                }
                let p =
                    pending.get_or_insert_with(|| PendingContainer::new_array(k as u16, split_idx));
                let lo_bytes = &raw_bytes[..split_idx * 2];
                match &mut p.store {
                    PendingStore::Array(vec) if vec.len() + split_idx <= ARRAY_LIMIT => {
                        vec.reserve(split_idx);
                        for chunk in lo_bytes.chunks_exact(2) {
                            let v = u16::from_le_bytes([chunk[0], chunk[1]]);
                            vec.push(v + in_offset);
                        }
                    }
                    _ => {
                        let (card, words) = p.ensure_bitmap();
                        for chunk in lo_bytes.chunks_exact(2) {
                            let v = u16::from_le_bytes([chunk[0], chunk[1]]) + in_offset;
                            words[(v >> 6) as usize] |= 1u64 << (v & 63);
                        }
                        *card += split_idx as u64;
                    }
                }
            }

            if need_hi {
                flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                let hi_len = cardinality - split_idx;
                if hi_len > 0 {
                    let mut vec = Vec::with_capacity(hi_len);
                    for chunk in raw_bytes[split_idx * 2..].chunks_exact(2) {
                        let v = u16::from_le_bytes([chunk[0], chunk[1]]);
                        vec.push(v.wrapping_add(in_offset));
                    }
                    pending = Some(PendingContainer {
                        key: k_hi as u16,
                        store: PendingStore::Array(vec),
                    });
                }
            }
        } else {
            let raw_bytes: &[u8; BITMAP_BYTES] =
                buf[cursor..cursor + BITMAP_BYTES].try_into().unwrap();
            let b = (in_offset >> 6) as usize;
            let shift = (in_offset & 63) as u32;
            let mut lo_card = 0u64;

            if need_lo {
                if pending.as_ref().is_some_and(|p| p.key != k as u16) {
                    flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                }
                let p = pending.get_or_insert_with(|| PendingContainer::new_bitmap(k as u16));
                let (card, words) = p.ensure_bitmap();
                if shift == 0 {
                    for idx in 0..(BITMAP_LENGTH - b) {
                        let w = read_u64_le(raw_bytes, idx);
                        words[b + idx] = w;
                        lo_card += w.count_ones() as u64;
                    }
                } else {
                    let rshift = 64 - shift;
                    let mut prev = read_u64_le(raw_bytes, 0);
                    let w0 = prev << shift;
                    words[b] |= w0;
                    lo_card += w0.count_ones() as u64;
                    for idx in 1..(BITMAP_LENGTH - b) {
                        let curr = read_u64_le(raw_bytes, idx);
                        let w = (curr << shift) | (prev >> rshift);
                        words[b + idx] = w;
                        lo_card += w.count_ones() as u64;
                        prev = curr;
                    }
                }
                *card += lo_card;
            }

            if need_hi {
                flush_pending(&mut pending, &mut descriptions, &mut payload_buf);
                let mut hi_words = Box::new([0u64; BITMAP_LENGTH]);
                let mut hi_card = 0u64;
                if shift == 0 {
                    let start = BITMAP_LENGTH - b;
                    for idx in start..BITMAP_LENGTH {
                        let w = read_u64_le(raw_bytes, idx);
                        hi_words[idx - start] = w;
                        if !need_lo {
                            hi_card += w.count_ones() as u64;
                        }
                    }
                } else {
                    let rshift = 64 - shift;
                    let start = BITMAP_LENGTH - b;
                    let mut prev = read_u64_le(raw_bytes, start - 1);
                    for idx in start..BITMAP_LENGTH {
                        let curr = read_u64_le(raw_bytes, idx);
                        let w = (curr << shift) | (prev >> rshift);
                        hi_words[idx - start] = w;
                        if !need_lo {
                            hi_card += w.count_ones() as u64;
                        }
                        prev = curr;
                    }
                    let last_w = prev >> rshift;
                    hi_words[b] = last_w;
                    if !need_lo {
                        hi_card += last_w.count_ones() as u64;
                    }
                }
                if need_lo {
                    hi_card = (cardinality as u64) - lo_card;
                }
                if hi_card > 0 {
                    pending = Some(PendingContainer {
                        key: k_hi as u16,
                        store: PendingStore::Bitmap { card: hi_card, words: hi_words },
                    });
                }
            }
        }

        cursor += payload_len;
    }

    flush_pending(&mut pending, &mut descriptions, &mut payload_buf);

    if descriptions.is_empty() {
        return RoaringBitmap::new();
    }

    let new_size = descriptions.len();
    let header_len = 8 + new_size * 8;
    let mut header_buf = Vec::with_capacity(header_len);
    header_buf.extend_from_slice(&SERIAL_COOKIE_NO_RUNCONTAINER.to_le_bytes());
    header_buf.extend_from_slice(&(new_size as u32).to_le_bytes());
    for &(key, card_m1) in &descriptions {
        header_buf.extend_from_slice(&key.to_le_bytes());
        header_buf.extend_from_slice(&card_m1.to_le_bytes());
    }
    // Offset table: the absolute byte position of each container's payload. The in-memory
    // deserializer ignores it, but writing correct values keeps the buffer a valid portable
    // serialization rather than relying on that behavior.
    let mut payload_pos = header_len as u32;
    for &(_, card_m1) in &descriptions {
        header_buf.extend_from_slice(&payload_pos.to_le_bytes());
        let cardinality = (card_m1 as usize) + 1;
        payload_pos +=
            if cardinality <= ARRAY_LIMIT { (cardinality * 2) as u32 } else { BITMAP_BYTES as u32 };
    }
    debug_assert_eq!(payload_pos as usize, header_len + payload_buf.len());

    use std::io::Read;
    let reader = header_buf.as_slice().chain(payload_buf.as_slice());
    RoaringBitmap::deserialize_unchecked_from(reader)
        .unwrap_or_else(|_| add_offset_32_naive(bitmap, offset))
}

fn add_offset_64(treemap: &RoaringTreemap, offset: i64) -> RoaringTreemap {
    if offset == 0 || treemap.is_empty() {
        return treemap.clone();
    }
    let bucket_offset = offset >> 32;
    let lo_offset = offset & 0xFFFF_FFFF;
    let mut map: std::collections::BTreeMap<u32, RoaringBitmap> = std::collections::BTreeMap::new();
    for (hi, bitmap) in treemap.bitmaps() {
        let k = (hi as i64) + bucket_offset;
        if (0..=u32::MAX as i64).contains(&k) {
            let shifted_lo =
                if lo_offset == 0 { bitmap.clone() } else { add_offset_32(bitmap, lo_offset) };
            if !shifted_lo.is_empty() {
                map.entry(k as u32)
                    .and_modify(|existing| *existing |= &shifted_lo)
                    .or_insert(shifted_lo);
            }
        }
        if lo_offset != 0 {
            let k_hi = k + 1;
            if (0..=u32::MAX as i64).contains(&k_hi) {
                let shifted_hi = add_offset_32(bitmap, lo_offset - (1i64 << 32));
                if !shifted_hi.is_empty() {
                    map.insert(k_hi as u32, shifted_hi);
                }
            }
        }
    }
    RoaringTreemap::from_bitmaps(map)
}

#[cfg(test)]
mod add_offset_tests {
    use super::{add_offset_32, add_offset_32_naive, add_offset_64};
    use roaring::{RoaringBitmap, RoaringTreemap};

    /// Minimal xorshift64* generator so the tests need no extra dependencies.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// Element-by-element reference for `add_offset_64`.
    fn add_offset_64_naive(treemap: &RoaringTreemap, offset: i64) -> RoaringTreemap {
        if offset >= 0 {
            let offset = offset as u64;
            treemap.iter().take_while(|&x| x <= u64::MAX - offset).map(|x| x + offset).collect()
        } else {
            let neg = offset.unsigned_abs();
            treemap.iter().filter(|&x| x >= neg).map(|x| x - neg).collect()
        }
    }

    /// Builds a bitmap whose containers span the array/bitmap/run/full store types, including
    /// containers at keys 0 and 0xFFFF and adjacent key pairs so that shifted halves merge.
    fn random_bitmap(rng: &mut Rng) -> RoaringBitmap {
        let mut bm = RoaringBitmap::new();
        let mut keys: Vec<u32> =
            (0..1 + rng.below(5)).map(|_| rng.below(0x1_0000) as u32).collect();
        if rng.below(4) == 0 {
            keys.push(0);
        }
        if rng.below(4) == 0 {
            keys.push(0xFFFF);
        }
        if rng.below(3) == 0 {
            let k = (rng.below(0x1_0000) as u32) & 0xFFFE;
            keys.push(k);
            keys.push(k + 1);
        }
        for key in keys {
            let base = key << 16;
            match rng.below(5) {
                0 => {
                    for _ in 0..1 + rng.below(300) {
                        bm.insert(base + rng.below(0x1_0000) as u32);
                    }
                }
                1 => {
                    // Straddles the 4096 array/bitmap boundary.
                    for _ in 0..3500 + rng.below(1200) {
                        bm.insert(base + rng.below(0x1_0000) as u32);
                    }
                }
                2 => {
                    for _ in 0..5000 + rng.below(15000) {
                        bm.insert(base + rng.below(0x1_0000) as u32);
                    }
                }
                3 => {
                    for _ in 0..1 + rng.below(4) {
                        let s = rng.below(0x1_0000) as u32;
                        let e = (s + 1 + rng.below(3000) as u32).min(0xFFFF);
                        bm.insert_range(base + s..=base + e);
                    }
                    if rng.below(2) == 0 {
                        bm.insert_range(base..=base + rng.below(100) as u32);
                    }
                    if rng.below(2) == 0 {
                        bm.insert_range(base + 0xFFFF - rng.below(100) as u32..=base + 0xFFFF);
                    }
                }
                _ => {
                    bm.insert_range(base..=base + 0xFFFF);
                }
            }
        }
        if rng.below(2) == 0 {
            bm.optimize();
        }
        bm
    }

    /// Serializes `bm` and reads it back through the *checked* deserializer, which validates
    /// that every store is well-formed (sorted arrays, correct bitmap cardinalities).
    fn assert_well_formed(bm: &RoaringBitmap) {
        let mut bytes = Vec::new();
        bm.serialize_into(&mut bytes).unwrap();
        let round_trip = RoaringBitmap::deserialize_from(&bytes[..]).unwrap();
        assert_eq!(&round_trip, bm);
    }

    const SPECIAL_OFFSETS_32: [i64; 18] = [
        1,
        -1,
        63,
        64,
        65,
        -63,
        -64,
        -65,
        65535,
        -65535,
        65536,
        -65536,
        65537,
        -65537,
        (1 << 32) - 1,
        -((1 << 32) - 1),
        1 << 31,
        -(1 << 31),
    ];

    #[test]
    fn add_offset_32_matches_reference() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..150 {
            let bm = random_bitmap(&mut rng);
            let mut offsets = SPECIAL_OFFSETS_32.to_vec();
            for _ in 0..6 {
                offsets.push(rng.below(1 << 33) as i64 - (1 << 32));
                offsets.push(rng.below(200_000) as i64 - 100_000);
            }
            for offset in offsets {
                let expected = add_offset_32_naive(&bm, offset);
                let actual = add_offset_32(&bm, offset);
                assert_eq!(actual, expected, "offset={offset} len={}", bm.len());
                assert_well_formed(&actual);
            }
        }
    }

    #[test]
    fn add_offset_32_handles_edge_cases() {
        let empty = RoaringBitmap::new();
        assert!(add_offset_32(&empty, 5).is_empty());
        let full_container: RoaringBitmap = (0..=0xFFFF).collect();
        assert_eq!(add_offset_32(&full_container, 0), full_container);
        assert!(add_offset_32(&full_container, u32::MAX as i64 + 1).is_empty());
        assert!(add_offset_32(&full_container, -(u32::MAX as i64) - 1).is_empty());
        assert!(add_offset_32(&full_container, -0x1_0000).is_empty());
        let shifted = add_offset_32(&full_container, 0xFFFF_0000);
        assert_eq!(shifted.min(), Some(0xFFFF_0000));
        assert_eq!(shifted.len(), 0x1_0000);
    }

    #[test]
    fn add_offset_64_matches_reference() {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        for _ in 0..40 {
            let mut tm = RoaringTreemap::new();
            for _ in 0..1 + rng.below(3) {
                let hi = match rng.below(5) {
                    0 => rng.below(3) as u32,
                    1 => u32::MAX,
                    _ => rng.below(1 << 32) as u32,
                };
                let part = random_bitmap(&mut rng);
                for v in part.iter() {
                    tm.insert(((hi as u64) << 32) | v as u64);
                }
                if rng.below(2) == 0 && hi < u32::MAX {
                    for v in part.iter() {
                        tm.insert((((hi + 1) as u64) << 32) | (v ^ 0x5555) as u64);
                    }
                }
            }
            let mut offsets = vec![
                1,
                -1,
                1 << 32,
                -(1 << 32),
                (1 << 32) + 7,
                -(1 << 32) - 7,
                (1 << 32) - 1,
                -((1 << 32) - 1),
                i64::MAX,
                i64::MIN,
                1 << 40,
                -(1 << 40),
            ];
            for _ in 0..4 {
                offsets.push(rng.next() as i64);
                offsets.push(rng.below(1 << 34) as i64 - (1 << 33));
            }
            for offset in offsets {
                let expected = add_offset_64_naive(&tm, offset);
                let actual = add_offset_64(&tm, offset);
                assert_eq!(actual, expected, "offset={offset}");
                for (_, bm) in actual.bitmaps() {
                    assert_well_formed(bm);
                }
            }
        }
    }
}
