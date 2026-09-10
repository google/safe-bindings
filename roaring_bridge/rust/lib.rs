//! Crubit-friendly wrapper around the `roaring` crate.
//!
//! This crate provides `RoaringBitmap32` (32-bit) and `RoaringBitmap64` (64-bit)
//! roaring bitmap types with explicit, non-generic method signatures that Crubit
//! can automatically expose to C++.
//!
//! The C++ namespace is `roaring_bridge`.

use roaring::{RoaringBitmap, RoaringTreemap};

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

    /// Collects all values into a vector.
    pub fn ToVec(&self) -> vector<u32> {
        self.inner.iter().collect()
    }

    /// Returns the serialized size in bytes.
    pub fn serialized_size(&self) -> usize {
        self.inner.serialized_size()
    }

    /// Serializes to bytes (standard Roaring format, compatible with CRoaring).
    pub fn Serialize(&self) -> vector<u8> {
        let mut buf = Vec::with_capacity(self.inner.serialized_size());
        self.inner.serialize_into(&mut buf).expect("serialization to Vec should not fail");
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
    pub fn Serialize(&self) -> vector<u8> {
        let mut buf = Vec::with_capacity(self.inner.serialized_size());
        self.inner.serialize_into(&mut buf).expect("serialization to Vec should not fail");
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
