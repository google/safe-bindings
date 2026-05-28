#include <iostream>
#include <ostream>
#include <string>
#include <utility>

#include "flate2.h"
#include "gzip_wrapper.h"

int main(int argc, char** argv) {
  std::string input = "hello world, gzip wrapper example!";

  std::cout << "Original: " << input << std::endl;

  auto compressed = security::deflate::CompressGzip(input, 9);
  if (!compressed.ok()) {
    std::cerr << "Error compressing: " << compressed.status() << std::endl;
    return 1;
  }

  absl::Cord cord = std::move(compressed).value().as_cord();
  std::cout << "Compressed size: " << cord.size() << " bytes" << std::endl;

  auto decompressed = security::deflate::UncompressGzip(cord.Flatten());
  if (!decompressed.ok()) {
    std::cerr << "Error decompressing: " << decompressed.status() << std::endl;
    return 1;
  }

  std::cout << "Decompressed: " << decompressed->as_string_view() << std::endl;

  return 0;
}
