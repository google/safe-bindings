#include <algorithm>
#include <cstdint>
#include <numeric>
#include <optional>
#include <string>
#include <type_traits>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "absl/types/span.h"
#include "crubit/roaring_bridge.h"

namespace {

// Helper to convert Crubit-returned containers to std::vector
// for comparison.
template <typename Container>
auto ToStdVec(const Container& c)
    -> std::vector<std::decay_t<decltype(*c.begin())>> {
  return {c.begin(), c.end()};
}

using ::roaring_bridge::RoaringBitmap32;
using ::roaring_bridge::RoaringBitmap64;
using ::testing::Optional;

template <typename T>
struct BitmapTraits;

template <>
struct BitmapTraits<RoaringBitmap32> {
  using ValueType = uint32_t;
  static constexpr const char* kName = "RoaringBitmap32";
};

template <>
struct BitmapTraits<RoaringBitmap64> {
  using ValueType = uint64_t;
  static constexpr const char* kName = "RoaringBitmap64";
};

struct BitmapTypeNames {
  template <typename T>
  static std::string GetName(int) {
    return BitmapTraits<T>::kName;
  }
};

template <typename T>
class RoaringBitmapTest : public ::testing::Test {};

using BitmapTypes = ::testing::Types<RoaringBitmap32, RoaringBitmap64>;
TYPED_TEST_SUITE(RoaringBitmapTest, BitmapTypes, BitmapTypeNames);

// Parameterized tests for RoaringBitmap32 and RoaringBitmap64
// -----------------------------------------------------------

TYPED_TEST(RoaringBitmapTest, EmptyBitmap) {
  TypeParam bm;
  EXPECT_TRUE(bm.IsEmpty());
  EXPECT_EQ(bm.len(), 0);
}

TYPED_TEST(RoaringBitmapTest, InsertAndContains) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;
  TypeParam bm;
  EXPECT_TRUE(bm.Insert(1));
  bm.InsertMany(std::vector<ValueType>{2, 3});
  EXPECT_FALSE(bm.Insert(3));  // duplicate

  EXPECT_EQ(bm.len(), 3);
  EXPECT_TRUE(bm.Contains(1));
  EXPECT_TRUE(bm.Contains(2));
  EXPECT_TRUE(bm.Contains(3));
  EXPECT_FALSE(bm.Contains(4));
}

TYPED_TEST(RoaringBitmapTest, MinMax) {
  TypeParam bm;
  EXPECT_FALSE(bm.Min().has_value());
  EXPECT_FALSE(bm.Max().has_value());

  bm.Insert(10);
  bm.Insert(20);
  bm.Insert(30);

  EXPECT_EQ(*bm.Min(), 10);
  EXPECT_EQ(*bm.Max(), 30);
}

TYPED_TEST(RoaringBitmapTest, InsertRange) {
  TypeParam bm;
  EXPECT_EQ(bm.InsertRange(10, 20), 10);
  EXPECT_EQ(bm.len(), 10);
  EXPECT_TRUE(bm.Contains(10));
  EXPECT_TRUE(bm.Contains(19));
  EXPECT_FALSE(bm.Contains(20));
}

TYPED_TEST(RoaringBitmapTest, Remove) {
  TypeParam bm;
  bm.Insert(5);
  EXPECT_TRUE(bm.Remove(5));
  EXPECT_FALSE(bm.Remove(5));
  EXPECT_FALSE(bm.Contains(5));
}

TYPED_TEST(RoaringBitmapTest, Operators) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;

  TypeParam a;
  a.InsertRange(0, 10);

  TypeParam b;
  b.InsertRange(5, 15);

  TypeParam isect = a & b;
  EXPECT_EQ(isect.len(), 5);
  EXPECT_EQ(ToStdVec(isect.ToVec()), (std::vector<ValueType>{5, 6, 7, 8, 9}));

  TypeParam un = a | b;
  EXPECT_EQ(un.len(), 15);
  EXPECT_EQ(ToStdVec(un.ToVec()),
            (std::vector<ValueType>{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
                                    13, 14}));

  TypeParam diff = a - b;
  EXPECT_EQ(diff.len(), 5);
  EXPECT_EQ(ToStdVec(diff.ToVec()), (std::vector<ValueType>{0, 1, 2, 3, 4}));

  TypeParam xor_result = a ^ b;
  EXPECT_EQ(xor_result.len(), 10);
  EXPECT_EQ(ToStdVec(xor_result.ToVec()),
            (std::vector<ValueType>{0, 1, 2, 3, 4, 10, 11, 12, 13, 14}));

  // a and b are still valid (not consumed).
  EXPECT_EQ(a.len(), 10);
  EXPECT_EQ(b.len(), 10);
}

TYPED_TEST(RoaringBitmapTest, InPlaceOperators) {
  TypeParam a;
  a.InsertRange(0, 10);

  TypeParam b;
  b.InsertRange(5, 15);

  EXPECT_EQ(a.IntersectionLen(b), 5);
  EXPECT_EQ(a.UnionLen(b), 15);

  TypeParam isect = TypeParam(a);
  isect &= b;
  EXPECT_EQ(isect.len(), 5);

  TypeParam un = TypeParam(a);
  un |= b;
  EXPECT_EQ(un.len(), 15);

  TypeParam diff = TypeParam(a);
  diff -= b;
  EXPECT_EQ(diff.len(), 5);

  TypeParam sym_diff = TypeParam(a);
  sym_diff ^= b;
  EXPECT_EQ(sym_diff.len(), 10);
}

TYPED_TEST(RoaringBitmapTest, MultiOps) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;

  std::vector<TypeParam> bitmaps(3);
  bitmaps[0].InsertRange(0, 10);
  bitmaps[1].InsertRange(5, 15);
  bitmaps[2].InsertRange(8, 20);

  TypeParam un = TypeParam::Union(bitmaps);
  EXPECT_EQ(un.len(), 20);
  EXPECT_EQ(un, bitmaps[0] | bitmaps[1] | bitmaps[2]);

  TypeParam isect = TypeParam::Intersect(bitmaps);
  EXPECT_EQ(ToStdVec(isect.ToVec()), (std::vector<ValueType>{8, 9}));
  EXPECT_EQ(isect, bitmaps[0] & bitmaps[1] & bitmaps[2]);

  // Empty input yields an empty bitmap.
  std::vector<TypeParam> empty;
  EXPECT_TRUE(TypeParam::Union(empty).IsEmpty());
  EXPECT_TRUE(TypeParam::Intersect(empty).IsEmpty());
}

TYPED_TEST(RoaringBitmapTest, MultiOpsWithAbslSpan) {
  std::vector<TypeParam> bitmaps(2);
  bitmaps[0].InsertRange(0, 10);
  bitmaps[1].InsertRange(5, 15);

  absl::Span<const TypeParam> span = absl::MakeConstSpan(bitmaps);
  EXPECT_EQ(TypeParam::Union(span).len(), 15);
  EXPECT_EQ(TypeParam::Intersect(span).len(), 5);

  // Subspans work too.
  EXPECT_EQ(TypeParam::Union(span.subspan(1)), bitmaps[1]);
  EXPECT_EQ(TypeParam::Intersect(span.subspan(1)), bitmaps[1]);
}

TYPED_TEST(RoaringBitmapTest, SerializationRoundtrip) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;
  TypeParam bm;
  bm.InsertRange(100, 200);
  bm.InsertMany(std::vector<ValueType>{1000, 50000});

  auto bytes = bm.Serialize();
  EXPECT_GT(bytes.size(), 0u);

  // Roundtrip via Deserialize (with validation).
  std::optional<TypeParam> restored = TypeParam::Deserialize(bytes);
  EXPECT_THAT(restored, Optional(bm));
  EXPECT_EQ(restored->len(), 102);
  EXPECT_TRUE(restored->Contains(100));
  EXPECT_TRUE(restored->Contains(199));
  EXPECT_TRUE(restored->Contains(1000));
  EXPECT_TRUE(restored->Contains(50000));
  EXPECT_FALSE(restored->Contains(99));

  // Roundtrip via DeserializeUnchecked (no validation, faster).
  EXPECT_THAT(TypeParam::DeserializeUnchecked(bytes), Optional(bm));

  // Invalid input returns std::nullopt.
  std::vector<uint8_t> garbage = {0xFF, 0x00, 0x42};
  EXPECT_FALSE(TypeParam::Deserialize(garbage).has_value());
}

TYPED_TEST(RoaringBitmapTest, RankAndSelect) {
  TypeParam bm;
  bm.Insert(10);
  bm.Insert(20);
  bm.Insert(30);

  EXPECT_EQ(bm.Rank(10), 1);
  EXPECT_EQ(bm.Rank(20), 2);
  EXPECT_EQ(bm.Rank(30), 3);

  EXPECT_TRUE(bm.Select(0).has_value());
  EXPECT_EQ(*bm.Select(0), 10);
  EXPECT_EQ(*bm.Select(1), 20);
  EXPECT_EQ(*bm.Select(2), 30);
  EXPECT_FALSE(bm.Select(3).has_value());
}

TYPED_TEST(RoaringBitmapTest, Subset) {
  TypeParam a;
  a.InsertRange(0, 10);

  TypeParam b;
  b.InsertRange(0, 20);

  EXPECT_TRUE(a.IsSubset(b));
  EXPECT_FALSE(b.IsSubset(a));
}

TYPED_TEST(RoaringBitmapTest, Equality) {
  TypeParam a;
  a.InsertRange(0, 10);

  TypeParam b;
  b.InsertRange(0, 10);

  EXPECT_EQ(a, b);

  b.Insert(100);
  EXPECT_NE(a, b);
}

TYPED_TEST(RoaringBitmapTest, Clear) {
  TypeParam bm;
  bm.InsertRange(0, 100);
  EXPECT_EQ(bm.len(), 100);

  bm.Clear();
  EXPECT_TRUE(bm.IsEmpty());
  EXPECT_EQ(bm.len(), 0);
}

TYPED_TEST(RoaringBitmapTest, ToVec) {
  TypeParam bm;
  bm.Insert(3);
  bm.Insert(1);
  bm.Insert(5);

  auto values = bm.ToVec();
  ASSERT_EQ(values.size(), 3u);
  // Values should be in sorted order.
  EXPECT_EQ(values[0], 1);
  EXPECT_EQ(values[1], 3);
  EXPECT_EQ(values[2], 5);
}

TYPED_TEST(RoaringBitmapTest, RangeForIteration) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;
  TypeParam bm;
  bm.Insert(3);
  bm.Insert(1);
  bm.Insert(5);

  std::vector<ValueType> values;
  for (ValueType v : bm) {
    values.push_back(v);
  }

  ASSERT_EQ(values.size(), 3u);
  EXPECT_EQ(values[0], 1);
  EXPECT_EQ(values[1], 3);
  EXPECT_EQ(values[2], 5);

  // bm is still valid (iterator borrows internally).
  EXPECT_EQ(bm.len(), 3);
}

TYPED_TEST(RoaringBitmapTest, ExplicitIterator) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;
  TypeParam bm;
  bm.Insert(3);
  bm.Insert(1);
  bm.Insert(5);

  std::vector<ValueType> values;
  for (auto it = bm.begin(); it != bm.end(); ++it) {
    values.push_back(*it);
  }
  ASSERT_EQ(values.size(), 3u);
  EXPECT_EQ(values[0], 1);
  EXPECT_EQ(values[1], 3);
  EXPECT_EQ(values[2], 5);
}

TYPED_TEST(RoaringBitmapTest, AddOffset) {
  using ValueType = typename BitmapTraits<TypeParam>::ValueType;
  TypeParam bm;
  bm.Insert(10);
  bm.Insert(100);
  bm.Insert(1000);

  // Shift by 0 returns an identical bitmap.
  TypeParam zero_shift = bm.AddOffset(0);
  EXPECT_EQ(zero_shift, bm);

  // Positive offset.
  TypeParam shifted_pos = bm.AddOffset(17);
  EXPECT_EQ(shifted_pos.len(), 3);
  EXPECT_TRUE(shifted_pos.Contains(27));
  EXPECT_TRUE(shifted_pos.Contains(117));
  EXPECT_TRUE(shifted_pos.Contains(1017));
  EXPECT_FALSE(shifted_pos.Contains(10));
  EXPECT_EQ(ToStdVec(shifted_pos.ToVec()),
            (std::vector<ValueType>{27, 117, 1017}));

  // Negative offset with partial drop (10 - 17 < 0 is dropped).
  TypeParam shifted_neg = bm.AddOffset(-17);
  EXPECT_EQ(shifted_neg.len(), 2);
  EXPECT_TRUE(shifted_neg.Contains(83));
  EXPECT_TRUE(shifted_neg.Contains(983));
  EXPECT_FALSE(shifted_neg.Contains(10));
  EXPECT_EQ(ToStdVec(shifted_neg.ToVec()), (std::vector<ValueType>{83, 983}));

  // Large negative offset drops all elements.
  TypeParam shifted_all_dropped = bm.AddOffset(-2000);
  EXPECT_TRUE(shifted_all_dropped.IsEmpty());
  EXPECT_EQ(shifted_all_dropped.len(), 0);
}

// RoaringBitmap64-specific tests (values > u32::MAX)
// --------------------------------------------------

TEST(RoaringBitmap64Test, InsertLargeValues) {
  RoaringBitmap64 bm;
  EXPECT_TRUE(bm.Insert(1));
  EXPECT_TRUE(bm.Insert(UINT64_MAX));
  EXPECT_EQ(bm.len(), 2);
  EXPECT_TRUE(bm.Contains(1));
  EXPECT_TRUE(bm.Contains(UINT64_MAX));
  EXPECT_FALSE(bm.Contains(2));
}

TEST(RoaringBitmap64Test, SerializationRoundtrip) {
  RoaringBitmap64 bm;
  bm.Insert(100);
  bm.Insert(uint64_t{1} << 32);  // > u32::MAX
  bm.Insert(UINT64_MAX);

  auto bytes = bm.Serialize();
  EXPECT_GT(bytes.size(), 0u);

  // Roundtrip via Deserialize (with validation).
  std::optional<RoaringBitmap64> restored = RoaringBitmap64::Deserialize(bytes);
  EXPECT_THAT(restored, Optional(bm));
  EXPECT_EQ(restored->len(), 3);
  EXPECT_TRUE(restored->Contains(100));
  EXPECT_TRUE(restored->Contains(uint64_t{1} << 32));
  EXPECT_TRUE(restored->Contains(UINT64_MAX));

  // Roundtrip via DeserializeUnchecked (no validation, faster).
  EXPECT_THAT(RoaringBitmap64::DeserializeUnchecked(bytes), Optional(bm));

  // Invalid input returns std::nullopt.
  std::vector<uint8_t> garbage = {0xFF, 0x00, 0x42};
  EXPECT_FALSE(RoaringBitmap64::Deserialize(garbage).has_value());
}

TEST(RoaringBitmap64Test, RangeForIteration) {
  RoaringBitmap64 bm;
  bm.Insert(100);
  bm.Insert(uint64_t{1} << 32);
  bm.Insert(42);

  std::vector<uint64_t> values;
  for (uint64_t v : bm) {
    values.push_back(v);
  }

  ASSERT_EQ(values.size(), 3u);
  EXPECT_EQ(values[0], 42);
  EXPECT_EQ(values[1], 100);
  EXPECT_EQ(values[2], uint64_t{1} << 32);

  // bm is still valid.
  EXPECT_EQ(bm.len(), 3);
}

TEST(RoaringBitmap64Test, ExplicitIterator) {
  RoaringBitmap64 bm;
  bm.Insert(100);
  bm.Insert(uint64_t{1} << 32);
  bm.Insert(42);

  std::vector<uint64_t> values;
  for (auto it = bm.begin(); it != bm.end(); ++it) {
    values.push_back(*it);
  }

  ASSERT_EQ(values.size(), 3u);
  EXPECT_EQ(values[0], 42);
  EXPECT_EQ(values[1], 100);
  EXPECT_EQ(values[2], uint64_t{1} << 32);

  // bm is still valid.
  EXPECT_EQ(bm.len(), 3);
}

TEST(RoaringBitmap32Test, AddOffsetBoundary) {
  RoaringBitmap32 bm;
  bm.Insert(0);
  bm.Insert(100);
  bm.Insert(UINT32_MAX - 10);
  bm.Insert(UINT32_MAX);

  // Offset causing overflow for elements near UINT32_MAX.
  RoaringBitmap32 shifted = bm.AddOffset(5);
  EXPECT_EQ(shifted.len(), 3);
  EXPECT_TRUE(shifted.Contains(5));
  EXPECT_TRUE(shifted.Contains(105));
  EXPECT_TRUE(shifted.Contains(UINT32_MAX - 5));
  EXPECT_FALSE(shifted.Contains(UINT32_MAX));  // overflowed and dropped

  // Offset > UINT32_MAX drops everything.
  EXPECT_TRUE(bm.AddOffset(int64_t{UINT32_MAX} + 1).IsEmpty());

  // Offset < -UINT32_MAX drops everything.
  EXPECT_TRUE(bm.AddOffset(-int64_t{UINT32_MAX} - 1).IsEmpty());

  // Offset == UINT32_MAX keeps only 0 (shifted to UINT32_MAX).
  RoaringBitmap32 max_pos = bm.AddOffset(UINT32_MAX);
  EXPECT_EQ(max_pos.len(), 1);
  EXPECT_TRUE(max_pos.Contains(UINT32_MAX));

  // Offset == -UINT32_MAX keeps only UINT32_MAX (shifted to 0).
  RoaringBitmap32 max_neg = bm.AddOffset(-int64_t{UINT32_MAX});
  EXPECT_EQ(max_neg.len(), 1);
  EXPECT_TRUE(max_neg.Contains(0));
}

TEST(RoaringBitmap32Test, FirstAtOrAfter) {
  RoaringBitmap32 bm;
  EXPECT_EQ(bm.FirstAtOrAfter(0), std::nullopt);
  EXPECT_EQ(bm.FirstAtOrAfter(UINT32_MAX), std::nullopt);

  // An array container, a run container, a bitmap container, and the last
  // possible value, with container key 2 left empty.
  bm.InsertMany(std::vector<uint32_t>{10, 20});
  bm.InsertRange(uint32_t{1} << 16, (uint32_t{1} << 16) + 100);
  // Fills the bitmap container with every even value of container key 3.
  for (uint32_t value = uint32_t{3} << 16; value < (uint32_t{4} << 16);
       value += 2) {
    bm.Insert(value);
  }
  bm.Insert(UINT32_MAX);
  bm.Optimize();

  // Array container: exact hits and the next value in the same container.
  EXPECT_THAT(bm.FirstAtOrAfter(0), Optional(10));
  EXPECT_THAT(bm.FirstAtOrAfter(10), Optional(10));
  EXPECT_THAT(bm.FirstAtOrAfter(11), Optional(20));
  EXPECT_THAT(bm.FirstAtOrAfter(21), Optional(uint32_t{1} << 16));

  // Run container: inside the run, and past its end into the empty key 2.
  EXPECT_THAT(bm.FirstAtOrAfter((uint32_t{1} << 16) + 50),
              Optional((uint32_t{1} << 16) + 50));
  EXPECT_THAT(bm.FirstAtOrAfter((uint32_t{1} << 16) + 100),
              Optional(uint32_t{3} << 16));
  EXPECT_THAT(bm.FirstAtOrAfter(uint32_t{2} << 16),
              Optional(uint32_t{3} << 16));

  // Bitmap container: an exact hit, the next value, and its last odd value.
  EXPECT_THAT(bm.FirstAtOrAfter((uint32_t{3} << 16) + 2),
              Optional((uint32_t{3} << 16) + 2));
  EXPECT_THAT(bm.FirstAtOrAfter((uint32_t{3} << 16) + 3),
              Optional((uint32_t{3} << 16) + 4));
  EXPECT_THAT(bm.FirstAtOrAfter((uint32_t{4} << 16) - 1), Optional(UINT32_MAX));

  // An absent container key, and the last possible value.
  EXPECT_THAT(bm.FirstAtOrAfter(uint32_t{1} << 20), Optional(UINT32_MAX));
  EXPECT_THAT(bm.FirstAtOrAfter(UINT32_MAX), Optional(UINT32_MAX));

  bm.Remove(UINT32_MAX);
  EXPECT_EQ(bm.FirstAtOrAfter((uint32_t{4} << 16) - 1), std::nullopt);
}

TEST(RoaringBitmap32Test, CopyValuesFromReadsInBatches) {
  RoaringBitmap32 bm;
  bm.InsertRange(100, 200);
  bm.InsertMany(std::vector<uint32_t>{70000, UINT32_MAX});

  // A full batch stops in the middle of the first run.
  std::vector<uint32_t> buffer(64, 42);
  ASSERT_EQ(bm.CopyValuesFrom(0, absl::MakeSpan(buffer)), 64);
  std::vector<uint32_t> expected(64);
  std::iota(expected.begin(), expected.end(), 100);
  EXPECT_EQ(buffer, expected);

  // Resuming one past the last value copied returns the remaining 38 values
  // and leaves the rest of the buffer untouched.
  std::fill(buffer.begin(), buffer.end(), 42);
  ASSERT_EQ(bm.CopyValuesFrom(164, absl::MakeSpan(buffer)), 38);
  expected.assign(64, 42);
  std::iota(expected.begin(), expected.begin() + 36, 164);
  expected[36] = 70000;
  expected[37] = UINT32_MAX;
  EXPECT_EQ(buffer, expected);

  // A start past the last value of its container, and a start in an absent
  // container key, both continue with the next container.
  std::fill(buffer.begin(), buffer.end(), 42);
  ASSERT_EQ(bm.CopyValuesFrom(1000, absl::MakeSpan(buffer)), 2);
  EXPECT_EQ(buffer[0], 70000);
  EXPECT_EQ(buffer[1], UINT32_MAX);
  EXPECT_EQ(buffer[2], 42);
  ASSERT_EQ(bm.CopyValuesFrom(uint32_t{2} << 16, absl::MakeSpan(buffer)), 1);
  EXPECT_EQ(buffer[0], UINT32_MAX);

  EXPECT_EQ(bm.CopyValuesFrom(UINT32_MAX, absl::MakeSpan(buffer)), 1);
  EXPECT_EQ(buffer[0], UINT32_MAX);

  // A start past the maximum copies nothing.
  bm.Remove(UINT32_MAX);
  std::fill(buffer.begin(), buffer.end(), 42);
  EXPECT_EQ(bm.CopyValuesFrom(70001, absl::MakeSpan(buffer)), 0);
  EXPECT_EQ(buffer, std::vector<uint32_t>(64, 42));

  std::vector<uint32_t> empty_buffer;
  EXPECT_EQ(bm.CopyValuesFrom(0, absl::MakeSpan(empty_buffer)), 0);
  EXPECT_EQ(RoaringBitmap32().CopyValuesFrom(0, absl::MakeSpan(buffer)), 0);
}

TEST(RoaringBitmap32Test, CopyValuesFromReadsEveryContainerType) {
  RoaringBitmap32 bm;

  // Array container.
  bm.InsertMany(std::vector<uint32_t>{1, 5, 9, 1000});

  // Run container that covers part of its block.
  bm.InsertRange((uint32_t{2} << 16) + 10, (uint32_t{2} << 16) + 5000);

  // Fills a bitmap container with every even value of container key 3.
  for (uint32_t value = uint32_t{3} << 16; value < (uint32_t{4} << 16);
       value += 2) {
    bm.Insert(value);
  }
  bm.Insert(UINT32_MAX);
  bm.Optimize();

  std::vector<uint32_t> values;
  std::vector<uint32_t> buffer(7);
  uint32_t start = 0;
  // Reads the whole bitmap in batches of an odd size, so batch edges fall
  // inside every kind of container, resuming one past the last value read
  // until a short batch or UINT32_MAX shows that no values remain.
  while (true) {
    const auto count = bm.CopyValuesFrom(start, absl::MakeSpan(buffer));
    values.insert(values.end(), buffer.begin(), buffer.begin() + count);
    if (count < buffer.size() || values.back() == UINT32_MAX) {
      break;
    }
    start = values.back() + 1;
  }

  // The per-value iterator is an independent reference for the full contents.
  std::vector<uint32_t> expected;
  for (const uint32_t value : bm) {
    expected.push_back(value);
  }
  EXPECT_EQ(values, expected);
}

TEST(RoaringBitmap32Test, CopyRunsFrom) {
  RoaringBitmap32 bm;
  bm.InsertRange(100, 200);
  bm.Insert(300);
  // A run that crosses the boundary between the first two containers.
  bm.InsertRange(65530, 65546);
  bm.InsertRangeClosed(UINT32_MAX - 1, UINT32_MAX);

  std::vector<uint32_t> mins(8, 42);
  std::vector<uint32_t> maxes(8, 42);
  ASSERT_EQ(bm.CopyRunsFrom(0, absl::MakeSpan(mins), absl::MakeSpan(maxes)), 4);
  EXPECT_EQ(mins, (std::vector<uint32_t>{100, 300, 65530, UINT32_MAX - 1, 42,
                                         42, 42, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{199, 300, 65545, UINT32_MAX, 42, 42,
                                          42, 42}));

  // A start inside a run truncates only the first run.
  std::fill(mins.begin(), mins.end(), 42);
  std::fill(maxes.begin(), maxes.end(), 42);
  ASSERT_EQ(bm.CopyRunsFrom(150, absl::MakeSpan(mins), absl::MakeSpan(maxes)),
            4);
  EXPECT_EQ(mins, (std::vector<uint32_t>{150, 300, 65530, UINT32_MAX - 1, 42,
                                         42, 42, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{199, 300, 65545, UINT32_MAX, 42, 42,
                                          42, 42}));

  // A start in a gap begins at the next run.
  std::fill(mins.begin(), mins.end(), 42);
  std::fill(maxes.begin(), maxes.end(), 42);
  ASSERT_EQ(bm.CopyRunsFrom(201, absl::MakeSpan(mins), absl::MakeSpan(maxes)),
            3);
  EXPECT_EQ(mins, (std::vector<uint32_t>{300, 65530, UINT32_MAX - 1, 42, 42, 42,
                                         42, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{300, 65545, UINT32_MAX, 42, 42, 42,
                                          42, 42}));

  // The capacity is the shorter of the two buffers, and the longer buffer is
  // untouched past it.
  std::fill(mins.begin(), mins.end(), 42);
  std::vector<uint32_t> short_maxes(2, 42);
  ASSERT_EQ(
      bm.CopyRunsFrom(0, absl::MakeSpan(mins), absl::MakeSpan(short_maxes)), 2);
  EXPECT_EQ(mins, (std::vector<uint32_t>{100, 300, 42, 42, 42, 42, 42, 42}));
  EXPECT_EQ(short_maxes, (std::vector<uint32_t>{199, 300}));

  std::fill(mins.begin(), mins.end(), 42);
  std::fill(maxes.begin(), maxes.end(), 42);
  EXPECT_EQ(RoaringBitmap32().CopyRunsFrom(0, absl::MakeSpan(mins),
                                           absl::MakeSpan(maxes)),
            0);
  EXPECT_EQ(mins, std::vector<uint32_t>(8, 42));
  EXPECT_EQ(maxes, std::vector<uint32_t>(8, 42));
}

TEST(RoaringBitmap32Test, CopyRunsFromAtContainerBoundaries) {
  RoaringBitmap32 crossing;
  crossing.InsertRange(65530, 65546);
  std::vector<uint32_t> mins(2, 42);
  std::vector<uint32_t> maxes(2, 42);

  // Starts inside a run that crosses a container boundary, on either side of
  // the boundary.
  ASSERT_EQ(
      crossing.CopyRunsFrom(65533, absl::MakeSpan(mins), absl::MakeSpan(maxes)),
      1);
  EXPECT_EQ(mins, (std::vector<uint32_t>{65533, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{65545, 42}));
  ASSERT_EQ(
      crossing.CopyRunsFrom(65536, absl::MakeSpan(mins), absl::MakeSpan(maxes)),
      1);
  EXPECT_EQ(mins, (std::vector<uint32_t>{65536, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{65545, 42}));

  // A run that ends at the last value of a container does not merge with a
  // next container that lacks its first value.
  RoaringBitmap32 gap_after_boundary;
  gap_after_boundary.InsertMany(std::vector<uint32_t>{65535, 65537});
  std::fill(mins.begin(), mins.end(), 42);
  std::fill(maxes.begin(), maxes.end(), 42);
  ASSERT_EQ(gap_after_boundary.CopyRunsFrom(0, absl::MakeSpan(mins),
                                            absl::MakeSpan(maxes)),
            2);
  EXPECT_EQ(mins, (std::vector<uint32_t>{65535, 65537}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{65535, 65537}));

  // Nor does it merge across an absent container key.
  RoaringBitmap32 missing_key;
  missing_key.InsertRange((uint32_t{2} << 16) - 5, uint32_t{2} << 16);
  missing_key.Insert(uint32_t{3} << 16);
  ASSERT_EQ(
      missing_key.CopyRunsFrom(0, absl::MakeSpan(mins), absl::MakeSpan(maxes)),
      2);
  EXPECT_EQ(mins, (std::vector<uint32_t>{(uint32_t{2} << 16) - 5,
                                         uint32_t{3} << 16}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{(uint32_t{2} << 16) - 1,
                                          uint32_t{3} << 16}));
}

TEST(RoaringBitmap32Test, CopyRunsFromMergesFullContainers) {
  // Three full containers form a single run.
  RoaringBitmap32 bm;
  bm.InsertRange(0, uint32_t{3} << 16);
  bm.Optimize();

  std::vector<uint32_t> mins(2, 42);
  std::vector<uint32_t> maxes(2, 42);
  ASSERT_EQ(bm.CopyRunsFrom(0, absl::MakeSpan(mins), absl::MakeSpan(maxes)), 1);
  EXPECT_EQ(mins, (std::vector<uint32_t>{0, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{(uint32_t{3} << 16) - 1, 42}));

  // The full value range is a single run that ends at UINT32_MAX.
  ASSERT_EQ(RoaringBitmap32::Full().CopyRunsFrom(0, absl::MakeSpan(mins),
                                                 absl::MakeSpan(maxes)),
            1);
  EXPECT_EQ(mins, (std::vector<uint32_t>{0, 42}));
  EXPECT_EQ(maxes, (std::vector<uint32_t>{UINT32_MAX, 42}));
}

TEST(RoaringBitmap64Test, AddOffsetLargeValues) {
  RoaringBitmap64 bm;
  bm.Insert(100);
  bm.Insert(uint64_t{1} << 32);
  bm.Insert(UINT64_MAX - 10);

  RoaringBitmap64 shifted = bm.AddOffset(5);
  EXPECT_EQ(shifted.len(), 3);
  EXPECT_TRUE(shifted.Contains(105));
  EXPECT_TRUE(shifted.Contains((uint64_t{1} << 32) + 5));
  EXPECT_TRUE(shifted.Contains(UINT64_MAX - 5));

  RoaringBitmap64 overflowed = bm.AddOffset(20);
  EXPECT_EQ(overflowed.len(), 2);
  EXPECT_TRUE(overflowed.Contains(120));
  EXPECT_TRUE(overflowed.Contains((uint64_t{1} << 32) + 20));
  EXPECT_FALSE(overflowed.Contains(UINT64_MAX - 10));  // overflowed and dropped
}

}  // namespace
