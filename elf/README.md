# ELF

C++ wrapper for the [elf](https://crates.io/crates/elf) crate, for parsing
ELF (Executable and Linkable Format) files.

## Usage

Requires installation of clang and a nightly version of Rust.

```
rustup default nightly-2026-08-20
rustup component add rust-src rustc-dev llvm-tools-preview

cd elf
cmake -B build
cmake --build build --parallel

# Inspects its own executable if no path is given.
build/elf_example [/path/to/binary]
```

## Examples

See [main.cc](main.cc) for a complete example. The API lives in
[elf_bridge.h](elf_bridge.h), and [dump.h](dump.h) provides human-readable
dumps similar to `readelf`.

### Parsing a file and reading the header

`ElfBytes::Parse` parses an ELF file that is already in memory. Every accessor
returns an `absl::StatusOr`, so malformed input is reported as an error rather
than causing undefined behavior.

```cpp
#include "elf_bridge.h"

absl::StatusOr<elf_bridge::ElfBytes> elf =
    elf_bridge::ElfBytes::Parse(file_contents);
if (!elf.ok()) return elf.status();

absl::StatusOr<elf_bridge::FileHeader> header = elf->GetFileHeader();
if (!header.ok()) return header.status();

bool is_64_bit = header->GetClass() == elf_bridge::ElfClass::kElf64;
uint64_t entry_point = header->GetEntry();
```

### Finding a section by name

```cpp
absl::StatusOr<std::optional<elf_bridge::SectionHeader>> text =
    elf->GetSectionHeaderByName(".text");
if (text.ok() && text->has_value()) {
  absl::StatusOr<elf_bridge::SectionData> data = elf->GetSectionData(**text);
  // data->GetData() is an absl::string_view over the section contents.
}
```

### Iterating symbols

```cpp
absl::StatusOr<std::optional<elf_bridge::SymbolTableWithStrings>> symtab =
    elf->GetSymbolTable();
if (symtab.ok() && symtab->has_value()) {
  const elf_bridge::SymbolTable& symbols = (*symtab)->GetSymbolTable();
  const elf_bridge::StringTable& names = (*symtab)->GetStringTable();
  for (size_t i = 0; i < symbols.size(); ++i) {
    absl::StatusOr<elf_bridge::Symbol> symbol = symbols.GetSymbol(i);
    if (!symbol.ok()) continue;
    absl::StatusOr<std::string> name = names.Get(symbol->GetName());
    // symbol->GetValue(), symbol->GetSize(), symbol->GetType(), ...
  }
}
```

### Streaming large files

`ElfStream` reads from disk on demand instead of loading the whole file into
memory.

```cpp
absl::StatusOr<elf_bridge::ElfStream> stream =
    elf_bridge::ElfStream::Open("/path/to/large/binary");
if (!stream.ok()) return stream.status();

absl::StatusOr<elf_bridge::SectionHeaderTable> sections =
    stream->GetSectionHeaders();
```
