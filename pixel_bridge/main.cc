#include <iostream>
#include <ostream>
#include <utility>

#include "pixel_bridge.h"

int main(int argc, char** argv) {
  if (argc < 2) {
    std::cout << "Usage: " << argv[0] << " <image_path>" << std::endl;
    return 1;
  }

  // Read the image file.
  auto reader = security::pixel_bridge::ImageReader::NewFromFile(argv[1]);
  if (!reader.ok()) {
    std::cerr << "Error reading image: " << reader.status() << std::endl;
    return 1;
  }

  // Convert reader to decoder.
  auto decoder = std::move(reader).value().IntoDecoder();
  if (!decoder.ok()) {
    std::cerr << "Error decoding image: " << decoder.status() << std::endl;
    return 1;
  }

  // Print some basic info.
  std::cout << "Image Dimensions: " << decoder->GetWidth() << "x"
            << decoder->GetHeight() << std::endl;

  // Try to read EXIF metadata.
  auto exif = decoder->GetExifMetadata().value();
  if (exif.has_value()) {
    std::cout << "EXIF metadata size: " << exif->size() << " bytes"
              << std::endl;
  } else {
    std::cout << "No EXIF metadata found." << std::endl;
  }

  return 0;
}
