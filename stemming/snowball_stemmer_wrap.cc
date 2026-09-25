// Copied form google3/third_party/porter_stemmer_2013/porter-stemmer-wrap.cc
// Wrap Snowball stemmer for Google use.

#include "snowball_stemmer_wrap.h"

#include <cstddef>
#include <string>

#include "libstemmer.h"
#include "absl/strings/string_view.h"

using snowball_stemmer_c::sb_stemmer_delete;
using snowball_stemmer_c::sb_stemmer_length;
using snowball_stemmer_c::sb_stemmer_new;
using snowball_stemmer_c::sb_stemmer_stem;
using snowball_stemmer_c::sb_symbol;

namespace security::stemmer {

SnowballStemmer::SnowballStemmer(const char* lang_code)
    : stemmer_(sb_stemmer_new(lang_code, nullptr /* UTF-8 */)),
      lang_code_(lang_code) {}

SnowballStemmer::~SnowballStemmer() { sb_stemmer_delete(stemmer_); }

bool SnowballStemmer::IsSupportedLanguage() const {
  return stemmer_ != nullptr;
}

bool SnowballStemmer::StemUTF8(const absl::string_view &input,
                               std::string *output) {
  if (stemmer_ == nullptr || input.size() > kMaxStemTermLength) {
    output->assign(input.data(), input.size());
    return false;
  }

  const sb_symbol *stem = sb_stemmer_stem(
      stemmer_, reinterpret_cast<const sb_symbol *>(input.data()),
      static_cast<int>(input.size()));

  if (stem == nullptr) {
    output->assign(input.data(), input.size());
    return false;
  }

  output->assign(reinterpret_cast<const char *>(stem),
                 static_cast<size_t>(sb_stemmer_length(stemmer_)));
  return true;
}

}  // namespace security::stemmer
