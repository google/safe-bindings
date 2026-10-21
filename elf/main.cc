// Minimal example of the elf_bridge C++ API, a safe wrapper around the Rust
// `elf` crate: parses an ELF file and prints its file header.
//
// Usage: elf_example [path-to-elf-file]
//
// If no path is given, the example inspects its own executable.

#include <fstream>
#include <iostream>
#include <iterator>
#include <string>

#include "elf_bridge.h"
#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/str_format.h"
#include "absl/strings/string_view.h"

namespace {

absl::StatusOr<std::string> ReadFile(absl::string_view path) {
  std::ifstream file{std::string(path), std::ios::binary};
  if (!file.is_open()) {
    return absl::NotFoundError(absl::StrCat("Could not open ", path));
  }
  return std::string((std::istreambuf_iterator<char>(file)),
                     std::istreambuf_iterator<char>());
}

absl::string_view ClassName(elf_bridge::ElfClass elf_class) {
  switch (elf_class) {
    case elf_bridge::ElfClass::kElf32:
      return "ELF32";
    case elf_bridge::ElfClass::kElf64:
      return "ELF64";
    default:
      return "Unknown";
  }
}

}  // namespace

int main(int argc, char** argv) {
  const std::string path = argc > 1 ? argv[1] : "/proc/self/exe";

  absl::StatusOr<std::string> data = ReadFile(path);
  if (!data.ok()) {
    std::cerr << data.status() << "\n";
    return 1;
  }
  absl::StatusOr<elf_bridge::ElfBytes> elf = elf_bridge::ElfBytes::Parse(*data);
  if (!elf.ok()) {
    std::cerr << "Failed to parse ELF file: " << elf.status() << "\n";
    return 1;
  }
  absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
  if (!header.ok()) {
    std::cerr << "Failed to read file header: " << header.status() << "\n";
    return 1;
  }

  absl::StatusOr<elf_bridge::ElfMachine> machine = header->GetMachine();
  std::cout << "File:    " << path << "\n"
            << "Class:   " << ClassName(header->GetClass()) << "\n"
            << "Machine: "
            << (machine.ok() ? elf_bridge::MachineToHumanString(*machine)
                             : "unknown")
            << "\n"
            << "OS/ABI:  " << elf_bridge::OsAbiToString(header->GetOsAbi())
            << "\n"
            << "Entry:   " << absl::StrFormat("%#x", header->GetEntry())
            << "\n";
  return 0;
}
