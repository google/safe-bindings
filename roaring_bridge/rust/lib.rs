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
        if offset == 0 {
            return self.clone();
        }
        if offset > u32::MAX as i64 || offset < -(u32::MAX as i64) {
            return Self::default();
        }
        let inner = if offset > 0 {
            let offset = offset as u32;
            RoaringBitmap::from_sorted_iter(
                self.inner.range(..=(u32::MAX - offset)).map(|x| x + offset),
            )
            .expect("iterator is sorted")
        } else {
            let neg_offset = offset.unsigned_abs() as u32;
            RoaringBitmap::from_sorted_iter(self.inner.range(neg_offset..).map(|x| x - neg_offset))
                .expect("iterator is sorted")
        };
        Self { inner }
    }

    /// Collects all values into a vector.
    pub fn ToVec(&self) -> vector<u32> {
        let mut v = vector::default();
        v.resize(self.inner.len() as usize, 0);
        let _ = self.inner.iter().next_many(&mut v);
        v
    }

    /// Returns the smallest value that is >= `value`, or None if there is no such value.
    ///
    /// Locates the container holding `value` with a binary search, so the cost does not grow
    /// with the number of values before `value`. Returns the same value that CRoaring's
    /// `roaring_uint32_iterator_move_equalorlarger` moves an iterator to.
    pub fn FirstAtOrAfter(&self, value: u32) -> Option<u32> {
        self.inner.range(value..).next()
    }

    /// Copies the smallest values that are >= `start`, in ascending order, into `buffer`.
    ///
    /// Returns the number of values copied, which is the smaller of `buffer.len()` and the
    /// number of values >= `start`. Elements of `buffer` past the returned count are left
    /// untouched, and a count smaller than `buffer.len()` means that no values remain.
    ///
    /// To read the bitmap in batches, call this again with `start` set to one past the last
    /// value copied, stopping once that value is `u32::MAX`. Each call locates `start` with a
    /// binary search, so larger buffers spread that cost over more values. The values match
    /// what CRoaring's `roaring_uint32_iterator_read` returns from an iterator positioned at
    /// `start`.
    pub fn CopyValuesFrom(&self, start: u32, buffer: &mut [u32]) -> usize {
        self.inner.range(start..).next_many(buffer).len()
    }

    /// Copies the runs of consecutive values that are >= `start`, in ascending order.
    ///
    /// Run `i` covers the closed range [`mins[i]`, `maxes[i]`]; closed ranges let a run that ends
    /// at `u32::MAX` be represented without overflow. `mins` and `maxes` MUST NOT overlap: the
    /// generated C++ binding checks for aliasing and aborts the process if they do. Runs are
    /// maximal, including across container boundaries, except that the first run begins at
    /// `start` when `start` falls inside a run. A bitmap loaded with `DeserializeUnchecked` from
    /// data with adjacent runs inside one container can yield those runs separately.
    ///
    /// Returns the number of runs copied, which is at most `min(mins.len(), maxes.len())`.
    /// Elements past the returned count are left untouched, and a count smaller than that
    /// capacity means that no runs remain. To read the runs in batches, call this again with
    /// `start` set to one past the last copied `max`, stopping once that value is `u32::MAX`;
    /// because runs are maximal, this never splits a run. The runs match what CRoaring's
    /// `roaring_uint32_iterator_read_ranges` returns from an iterator positioned at `start`.
    pub fn CopyRunsFrom(&self, start: u32, mins: &mut [u32], maxes: &mut [u32]) -> usize {
        let mut iter = self.inner.range(start..);
        let mut count = 0;
        // Fills one (min, max) slot pair per run until either buffer is full or no runs remain.
        for (min, max) in mins.iter_mut().zip(maxes.iter_mut()) {
            let Some(run) = iter.next_range() else {
                break;
            };
            *min = *run.start();
            *max = *run.end();
            count += 1;
        }
        count
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
        if offset == 0 {
            return self.clone();
        }
        let inner = if offset > 0 {
            let offset = offset as u64;
            RoaringTreemap::from_sorted_iter(
                self.inner.iter().take_while(|&x| x <= u64::MAX - offset).map(|x| x + offset),
            )
            .expect("iterator is sorted")
        } else {
            let neg_offset = offset.unsigned_abs();
            let mut iter = self.inner.iter();
            iter.advance_to(neg_offset);
            RoaringTreemap::from_sorted_iter(iter.map(|x| x - neg_offset))
                .expect("iterator is sorted")
        };
        Self { inner }
    }

    /// Collects all values into a vector.
    pub fn ToVec(&self) -> vector<u64> {
        self.inner.iter().collect()
    }

    /// Copies the smallest values that are >= `start`, in ascending order, into `buffer`.
    ///
    /// Returns the number of values copied, which is the smaller of `buffer.len()` and the
    /// number of values >= `start`. Elements of `buffer` past the returned count are left
    /// untouched, and a count smaller than `buffer.len()` means that no values remain.
    ///
    /// To read the bitmap in batches, call this again with `start` set to one past the last
    /// value copied, stopping once that value is `u64::MAX`. Each call locates `start` by
    /// seeking directly to its bucket and container without scanning earlier values, so
    /// larger buffers spread that cost over more values.
    pub fn CopyValuesFrom(&self, start: u64, buffer: &mut [u64]) -> usize {
        let mut iter = self.inner.iter();
        iter.advance_to(start);
        let mut count = 0;
        for (slot, value) in buffer.iter_mut().zip(iter) {
            *slot = value;
            count += 1;
        }
        count
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
