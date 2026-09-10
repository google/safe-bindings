#include <cstdint>
#include <optional>
#include <string>
#include <type_traits>
#include <vector>

#include <gmock/gmock.h>
#include <gtest/gtest.h>
#include "roaring.h"

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

}  // namespace
