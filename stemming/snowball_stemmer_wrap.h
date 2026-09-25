// Copied from google3/third_party/porter_stemmer_2013/porter-stemmer-wrap.h
//
// Wraps a snowball stemmer for a given language.
// If the language is not supported, IsSupportedLanguage returns false and
// the StemUTF8 method is a pass-through.
//
// Porter stemmers are re-entrant but not thread safe, thus it is not safe
// to share PorterStemmer instances between threads.

#ifndef SECURITY_STEMMING_SNOWBALL_STEMMER_WRAP_H_
#define SECURITY_STEMMING_SNOWBALL_STEMMER_WRAP_H_

#include <cstddef>
#include <string>

#include "absl/strings/string_view.h"

namespace snowball_stemmer_c {
struct sb_stemmer;
}  // namespace snowball_stemmer_c

namespace security::stemmer {

// Input larger than this is skipped.
const size_t kMaxStemTermLength = 512;

class SnowballStemmer {
 public:
  // Instantiates a Snowball stemmer for the given language code.
  // For a list of supported languages, please refer to:
  //   https://source.corp.google.com/piper///depot/google3/security/stemming/libstemmer.rs;l=18-44
  // A Snowball stemmer object is re-entrant but is not thread safe.
  explicit SnowballStemmer(const char *lang_code);

  // This type is neither copyable nor movable.
  SnowballStemmer(const SnowballStemmer &) = delete;
  SnowballStemmer &operator=(const SnowballStemmer &) = delete;

  ~SnowballStemmer();

  // Returns true if the stemmer was correctly initialized and the language
  // is supported by an underlying Porter stemmer.
  // Returns false otherwise.
  bool IsSupportedLanguage() const;

  // Returns true if the underlying stemmer successfully processed the input
  // word, false otherwise.
  // In either case, the input word to be stemmed is copied to the output
  // string allowing for silent pass-through operation.
  bool StemUTF8(const absl::string_view &input, std::string *output);

  // Returns the lang_code passed during construction.
  inline const std::string &lang_code() const { return lang_code_; }

 private:
  struct snowball_stemmer_c::sb_stemmer *stemmer_;
  const std::string lang_code_;
};

}  // namespace security::stemmer

#endif  // SECURITY_STEMMING_SNOWBALL_STEMMER_WRAP_H_
