#include <string>

#include <gtest/gtest.h>
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"
#include "xmltree.h"

namespace security::xml {
namespace {

std::string Repeat(absl::string_view s, int n) {
  std::string out;
  for (int i = 0; i < n; ++i) absl::StrAppend(&out, s);
  return out;
}

// Encodes ASCII `s` as UTF-16 with a byte order mark.
std::string Utf16(absl::string_view s, bool big_endian) {
  std::string out = big_endian ? "\xFE\xFF" : "\xFF\xFE";
  for (char c : s) {
    if (big_endian) out.push_back('\0');
    out.push_back(c);
    if (!big_endian) out.push_back('\0');
  }
  return out;
}

TEST(XmlDocumentTest, ParseAcceptsDoctypeWithoutEntityDeclarations) {
  absl::StatusOr<XmlDocument> doc = XmlDocument::Parse(
      "<!DOCTYPE r [<!ELEMENT r (#PCDATA)>]><r>a &lt; &#98;</r>");
  ASSERT_TRUE(doc.ok()) << doc.status();
  EXPECT_EQ(doc->GetText(doc->Root()).value_or(""), "a < b");
}

TEST(XmlDocumentTest, ParseRejectsEntityDeclarations) {
  EXPECT_EQ(
      XmlDocument::Parse("<!DOCTYPE r [<!ENTITY e \"x\">]><r>&e;</r>")
          .status()
          .code(),
      absl::StatusCode::kInvalidArgument);
  EXPECT_EQ(XmlDocument::Parse("<!DOCTYPE r [<!ENTITY e \"x\">]><r/>")
                .status()
                .code(),
            absl::StatusCode::kInvalidArgument);
}

TEST(XmlDocumentTest, ParseRejectsNestedEntityReferences) {
  // Each `&a;` expands to 9 KB, so 1.3 KB of input would make 900 KB of text.
  std::string xml = absl::StrCat(
      "<!DOCTYPE r [<!ENTITY b \"", std::string(1000, 'b'),
      "\"><!ENTITY a \"", Repeat("&b;", 9), "\">]><r>", Repeat("&a;", 100),
      "</r>");
  EXPECT_EQ(XmlDocument::Parse(xml).status().code(),
            absl::StatusCode::kInvalidArgument);
}

TEST(XmlDocumentTest, ParseRejectsParameterEntityChain) {
  // Parameter entities are expanded as the DTD is read, before any element:
  // here e5 is 1 MB, and every further 57-byte level multiplies that by 10.
  std::string dtd = "<!ENTITY % e0 \"xxxxxxxxxx\">";
  for (int i = 1; i <= 5; ++i) {
    absl::StrAppend(&dtd, "<!ENTITY % e", i, " \"",
                    Repeat(absl::StrCat("%e", i - 1, ";"), 10), "\">");
  }
  EXPECT_EQ(XmlDocument::Parse(absl::StrCat("<!DOCTYPE r [", dtd, "]><r/>"))
                .status()
                .code(),
            absl::StatusCode::kInvalidArgument);
}

TEST(XmlDocumentTest, ParseRejectsUtf16EntityDeclarations) {
  for (bool big_endian : {false, true}) {
    SCOPED_TRACE(big_endian ? "UTF-16BE" : "UTF-16LE");
    absl::StatusOr<XmlDocument> doc =
        XmlDocument::Parse(Utf16("<r>x</r>", big_endian));
    ASSERT_TRUE(doc.ok()) << doc.status();
    EXPECT_EQ(doc->GetText(doc->Root()).value_or(""), "x");

    EXPECT_EQ(XmlDocument::Parse(
                  Utf16("<!DOCTYPE r [<!ENTITY e \"x\">]><r>&e;</r>",
                        big_endian))
                  .status()
                  .code(),
              absl::StatusCode::kInvalidArgument);
  }
}

}  // namespace
}  // namespace security::xml
