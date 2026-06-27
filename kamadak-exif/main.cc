#include <iostream>
#include <ostream>
#include <string>
#include <utility>
#include <vector>

#include "exif_bridge.h"

int main(int argc, char** argv) {
  std::vector<uint8_t> raw_exif = {'M',  'M',  0x00, 0x2a, 0x00, 0x00, 0x00,
                                   0x08, 0x00, 0x01, 0x01, 0x00, 0xff, 0xff,
                                   0x00, 0x00, 0x00, 0x01, 0x00, 0x14, 0x00,
                                   0x00, 0x00, 0x00, 0x00, 0x00};

  std::cout << "Parsing raw EXIF data (" << raw_exif.size() << " bytes)..."
            << std::endl;

  absl::StatusOr<security::exif_bridge::Exif> exif =
      security::exif_bridge::Reader().read_raw(raw_exif);
  if (!exif.ok()) {
    std::cerr << "Error parsing EXIF: " << exif.status() << std::endl;
    return 1;
  }

  std::cout << "EXIF parsed successfully!" << std::endl;
  std::cout << "Little endian: " << (exif->little_endian() ? "true" : "false")
            << std::endl;
  std::cout << "Fields count: " << exif->fields().size() << std::endl;

  return 0;
}
