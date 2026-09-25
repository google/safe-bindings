#ifndef SECURITY_STEMMING_LIBSTEMMER_H_
#define SECURITY_STEMMING_LIBSTEMMER_H_

namespace snowball_stemmer_c {

struct sb_stemmer;

using sb_symbol = char;

extern "C" {

void sb_stemmer_delete(sb_stemmer *stemmer);

int sb_stemmer_length(sb_stemmer *stemmer);

const char **sb_stemmer_list();

sb_stemmer *sb_stemmer_new(const char *algorithm, const char *charenc);

const sb_symbol *sb_stemmer_stem(sb_stemmer *stemmer, const sb_symbol *word,
                                 int size);

}  // extern "C"

}  // namespace snowball_stemmer_c

#endif  // SECURITY_STEMMING_LIBSTEMMER_H_
