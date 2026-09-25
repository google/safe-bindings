#include <iostream>
#include <string>

#include "snowball_stemmer_wrap.h"

// Runs stemming examples across multiple supported languages and prints the
// results to stdout.
void RunDemonstration() {
  struct StemExample {
    const char* lang;
    const char* word;
  };

  const StemExample examples[] = {
      {"en", "broadening"},
      {"en", "abattement"},
      {"fr", "majestueuse"},
      {"de", "kätzchen"},
      {"de", "halstücher"},
      {"es", "torpedearon"},
      {"nl", "lichtgevoeligheid"},
      {"it", "pronuncerà"},
      {"sv", "kloekornas"},
      {"da", "undertrykkerens"},
      {"fi", "innostuessaan"},
      {"pt", "quimioterápicos"},
      {"ru", "валяется"},
      {"id", "berpemandangan"},
      {"ta", "இக்கதையின்"},
      {"lt", "katėmis"},
  };

  std::cout << "=== Snowball Stemmer Demonstration ===\n";
  for (const StemExample& ex : examples) {
    security::stemmer::SnowballStemmer stemmer(ex.lang);
    if (!stemmer.IsSupportedLanguage()) {
      std::cerr << "Language '" << ex.lang << "' is not supported.\n";
      continue;
    }

    std::string output;
    if (stemmer.StemUTF8(ex.word, &output)) {
      std::cout << "[" << ex.lang << "] '" << ex.word << "' -> '" << output
                << "'\n";
    } else {
      std::cerr << "Failed to stem '" << ex.word << "' in " << ex.lang
                << "\n";
    }
  }
  std::cout << "=====================================\n";
}

int main(int argc, char** argv) {
  if (argc <= 1) {
    RunDemonstration();
    std::cout << "Usage: " << argv[0] << " <language_code> <word>\n";
    std::cout << "Example: " << argv[0] << " en powered\n";
    return 0;
  }

  if (argc < 3) {
    std::cerr << "Error: Missing word to stem.\n";
    std::cerr << "Usage: " << argv[0] << " <language_code> <word>\n";
    std::cerr << "Example: " << argv[0] << " en powered\n";
    return 1;
  }

  const char* const lang = argv[1];

  security::stemmer::SnowballStemmer stemmer(lang);
  if (!stemmer.IsSupportedLanguage()) {
    std::cerr << "Language '" << lang << "' is not supported.\n";
    return 1;
  }

  const char* const input = argv[2];

  std::string output;
  if (stemmer.StemUTF8(input, &output)) {
    std::cout << "Stem of '" << input << "' in " << lang << " is '" << output
              << "'\n";
  } else {
    std::cerr << "Failed to stem '" << input << "'\n";
  }

  return 0;
}
