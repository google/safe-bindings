#include <cstddef>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <vector>

#include "lzma.h"

// Helper function to read a file into memory
std::vector<uint8_t> ReadFile(const char* filename) {
  std::ifstream file(filename, std::ios::binary);
  if (!file.is_open()) {
    return {};
  }
  return std::vector<uint8_t>((std::istreambuf_iterator<char>(file)),
                              std::istreambuf_iterator<char>());
}

int main(int argc, char** argv) {
  std::cout << "=== LZMA Decompression Showcase ===\n\n";

  if (argc < 2) {
    std::cout << "Usage: " << argv[0] << " <compressed_file.xz>\n\n";
    std::cout << "=== Quick Utility Stats ===\n";
    std::cout << "LZMA Version: " << lzma_version_string() << "\n";
    std::cout << "CPU Threads:  " << lzma_cputhreads() << "\n";
    return 0;
  }

  std::cout << "Reading file: " << argv[1] << "\n";
  const std::vector<uint8_t> compressed = ReadFile(argv[1]);
  if (compressed.empty()) {
    std::cerr << "Error: Could not read file or file is empty.\n";
    return 1;
  }

  // 1. Initialize the decoder
  lzma_stream strm = LZMA_STREAM_INIT;
  // Passing 0 for flags uses default behavior.
  // UINT64_MAX means no memory limit constraint applied here.
  lzma_ret ret =
      lzma_stream_decoder(&strm, /*memlimit=*/UINT64_MAX, /*flags=*/0);
  if (ret != LZMA_OK) {
    std::cerr << "Error initializing decoder: " << ret << "\n";
    return 1;
  }

  // 2. Prepare decompression buffer
  std::vector<uint8_t> decompressed(1024);

  strm.next_in = compressed.data();
  strm.avail_in = compressed.size();
  strm.next_out = decompressed.data();
  strm.avail_out = decompressed.size();

  // 3. Decompress stream
  std::cout << "Decompressing...\n";
  do {
    ret = lzma_code(&strm, LZMA_FINISH);
    if (ret == LZMA_OK && strm.avail_out == 0) {
      const size_t old_size = decompressed.size();
      decompressed.resize(old_size * 2);
      strm.next_out = decompressed.data() + old_size;
      strm.avail_out = old_size;
    }
  } while (ret == LZMA_OK);

  if (ret != LZMA_STREAM_END) {
    std::cerr << "Decompression failed with code: " << ret << "\n";
    lzma_end(&strm);
    return 1;
  }

  // 4. Calculate total produced bytes
  const size_t decompressed_size = decompressed.size() - strm.avail_out;
  std::cout << "Success! Decompressed " << compressed.size()
            << " bytes into " << decompressed_size << " bytes.\n\n";

  // 5. Output the result (treating as text for demonstration)
  std::cout << "--- Decompressed Content Preview ---\n";
  std::cout.write(reinterpret_cast<const char*>(decompressed.data()),
                  decompressed_size);
  std::cout << "\n------------------------------------\n";

  // 6. Free resources
  lzma_end(&strm);

  return 0;
}

