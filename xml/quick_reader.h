#ifndef SECURITY_XML_QUICK_READER_H_
#define SECURITY_XML_QUICK_READER_H_

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>

// Workaround for Crubit bug with serde re-exports (b/262052635).
namespace serde_core::de {
template <typename T>
struct IntoDeserializer;
}
namespace serde::de {
template <typename T>
using IntoDeserializer = ::serde_core::de::IntoDeserializer<T>;
}

#include "crubit/rust.h"
#include "absl/base/attributes.h"
#include "absl/base/nullability.h"
#include "absl/log/die_if_null.h"
#include "absl/status/statusor.h"
#include "absl/strings/string_view.h"
#include "support/rs_std/vec.h"

namespace security::xml {

class QuickReader {
 public:
  // Ideally we would just use `rust::ReaderConfig`, but the way Crubit
  // generates it we cannot initialize it to any value (just the default one and
  // then set the fields).
  // LINT.IfChange(ReaderConfig)
  struct Config {
    // If true, inner and outer XML substrings will be filled in the start-tag
    // event.
    bool xml_substrings = true;
    // If true, the `QuickReader` will ignore namespaces in the XML string.
    bool ignore_namespaces = false;
    // If true, the `QuickReader` will expand self-closing tags to emit a start
    // and an end event. If false, a self-closing tag will just emit a start
    // event.
    bool expand_empty_elements = false;
    // If true, the `QuickReader` will always yield `xmlns` declarations as
    // attributes. This happens anyway if `ignore_namespaces` is `true`.
    bool yield_xmlns_attributes = false;
  };
  // LINT.ThenChange(
  //     rust/quick_reader.rs:ReaderConfig,
  //     quick_reader.cc:ReaderConfig
  // )

  struct Start;
  struct End;
  struct Text;
  struct CData;
  struct Comment;
  struct Decl;
  struct PI;
  struct DocType;
  struct GeneralRef;
  struct Eof {};

  using Event = std::variant<Start, End, Text, CData, Comment, Decl, PI,
                             DocType, GeneralRef, Eof>;

  // Accessor classes: used by the events to provide parsed data.
  class Data;
  class Name;
  class Attribute;
  class Attributes;

  // WARNING: `xml` is borrowed, not copied. The underlying XML buffer must
  // outlive the QuickReader instance and any events returned by it.
  static absl::StatusOr<QuickReader> Create(
      absl::string_view xml ABSL_ATTRIBUTE_LIFETIME_BOUND);
  static absl::StatusOr<QuickReader> CreateWithConfig(
      absl::string_view xml ABSL_ATTRIBUTE_LIFETIME_BOUND, Config config);

  // Provides the next event. When the parsing is complete, returns `Eof` and
  // keeps returning that afterwards.
  //
  // WARNING: Calling NextEvent() advances the streaming parser and updates
  // the reader's namespace state.
  // Lifetime: Events and their child accessor objects (Name, Attribute,
  // Attributes) hold non-owning pointers to the QuickReader's heap-allocated
  // parser state, so the QuickReader (or the instance it was moved into) must
  // outlive them. In addition, views returned by Data, Name, and Attribute
  // accessors borrow from cached buffers owned by that Event/Attribute object,
  // so the Event/Attribute (or the instance it was moved into) must outlive any
  // view obtained from it.
  //
  // Accessors are `&`-qualified with ABSL_ATTRIBUTE_LIFETIME_BOUND, so Clang
  // rejects (-Werror,-Wdangling) dangling initializations chained off a
  // temporary StatusOr<Event>. Three things are NOT diagnosed: assignment
  // into an already-declared variable, escape from an inner scope, and
  // laundering the view through an intervening unannotated function call.
  // Attributes::NextAttribute() has a further gap of its own; see there.
  // Bind the Event to a named local variable and copy anything that must
  // outlive the Event or the next NextEvent() call.
  absl::StatusOr<Event> NextEvent() ABSL_ATTRIBUTE_LIFETIME_BOUND;

 private:
  explicit QuickReader(absl::string_view xml,
                       std::unique_ptr<rust::QuickReader> reader)
      : xml_(xml), reader_(ABSL_DIE_IF_NULL(std::move(reader))) {}

  absl::string_view xml_;
  std::unique_ptr<rust::QuickReader> reader_;
};

class QuickReader::Data {
 public:
  // The returned `string_view` must not outlive `*this`. A safer way to do this
  // would be to return a `std::string`, but that would force an extra copy for
  // every call. There already is a copy made on the Rust side, for other
  // reasons (lifetimes of references to the reader from the event), but sadly
  // we cannot turn that directly into a `std::string` there, because inside
  // Rust one cannot construct a `std::string`. Therefore Rust always delivers a
  // `rs_std::Vec<uint8_t>`. Turning that into a `std::string` on the C++ side
  // would be the second copy.
  //
  // WARNING: The returned string_view is NOT null-terminated and may contain
  // embedded null characters (`\0`) if present in the XML text or CDATA. Do NOT
  // pass `.data()` to C APIs expecting null-terminated strings (`printf("%s")`,
  // `strcmp`). Construct a `std::string` first.
  absl::string_view Content() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view Content() && = delete;

 private:
  friend class QuickReader;

  explicit Data(rust::XmlEvent event) : event_(std::move(event)) {}

  rust::XmlEvent event_;
  // `content_` is lazily initialized to avoid copying the content if it is not
  // needed, which might frequently be the case.
  std::optional<rs_std::Vec<uint8_t>> content_;
};

class QuickReader::Name {
 public:
  // The returned `string_view`s must not outlive `*this`. A safer way to do
  // this would be to return a `std::string`, but that would force an extra copy
  // for every call. Even if there were just one call, there would be two
  // copies, because inside Rust one cannot construct a `std::string` so it's
  // always input -> `rs_std::Vec<uint8_t>` -> `std::string`.
  //
  // WARNING: The returned string_views are NOT null-terminated and may contain
  // embedded null characters (`\0`). Do NOT pass `.data()` to C-style APIs
  // expecting null-terminated strings.
  absl::string_view QualifiedName() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view QualifiedName() && = delete;

  absl::string_view LocalName() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view LocalName() && = delete;

  std::optional<absl::string_view> NamespacePrefix() &
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::optional<absl::string_view> NamespacePrefix() && = delete;

  absl::StatusOr<absl::string_view> ResolvedNamespace() &
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::StatusOr<absl::string_view> ResolvedNamespace() && = delete;

  bool LocalNameValid() const;
  bool PrefixValid() const;

 private:
  friend class QuickReader;

  // `reader` is not owned by the constructed object and must outlive it.
  Name(rust::XmlEvent event,
       rust::QuickReader* absl_nonnull reader
           ABSL_ATTRIBUTE_LIFETIME_BOUND)
      : reader_(ABSL_DIE_IF_NULL(reader)), event_(std::move(event)) {}

  rust::QuickReader* reader_;  // Not owned.
  rust::XmlEvent event_;
  // The following 4 fields are lazily initialized to avoid copying the content
  // if not needed, which might frequently be the case.
  std::optional<rs_std::Vec<uint8_t>> qualified_name_;
  std::optional<rs_std::Vec<uint8_t>> local_name_;
  // The double-optional is on purpose: the outer one is for the lazy retrieval.
  // The inner one matches the Rust API, which can signal that there was no
  // prefix (no ":" in the qualified name) as opposed to an empty prefix.
  std::optional<std::optional<rs_std::Vec<uint8_t>>> namespace_prefix_;
  std::optional<rs_std::Vec<uint8_t>> resolved_namespace_;
};

class QuickReader::Attribute {
 public:
  // WARNING: The returned string_views from Attribute methods are NOT
  // null-terminated and may contain embedded null characters (`\0`). Do NOT
  // pass `.data()` to C-style APIs expecting null-terminated strings.
  absl::string_view QualifiedName() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view QualifiedName() && = delete;

  // Returns the local name part of the key.
  absl::string_view LocalName() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view LocalName() && = delete;

  std::optional<absl::string_view> NamespacePrefix() &
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  std::optional<absl::string_view> NamespacePrefix() && = delete;

  absl::StatusOr<absl::string_view> ResolvedNamespace() &
      ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::StatusOr<absl::string_view> ResolvedNamespace() && = delete;

  absl::string_view Value() & ABSL_ATTRIBUTE_LIFETIME_BOUND;
  absl::string_view Value() && = delete;

  bool LocalNameValid() const;
  bool PrefixValid() const;

 private:
  friend class Attributes;

  Attribute(rust::Attribute attribute,
            rust::QuickReader* absl_nonnull reader
                ABSL_ATTRIBUTE_LIFETIME_BOUND)
      : reader_(ABSL_DIE_IF_NULL(reader)), attribute_(std::move(attribute)) {}

  rust::QuickReader* reader_;  // Not owned.
  rust::Attribute attribute_;
  // The following 5 fields are lazily initialized to avoid copying the content
  // if not needed, which might frequently be the case.
  std::optional<rs_std::Vec<uint8_t>> qualified_key_;
  std::optional<rs_std::Vec<uint8_t>> local_name_;
  // The double-optional is on purpose: the outer one is for the lazy retrieval.
  // The inner one matches the Rust API, which can signal that there was no
  // prefix (no ":" in the qualified name) as opposed to an empty prefix.
  std::optional<std::optional<rs_std::Vec<uint8_t>>> namespace_prefix_;
  std::optional<rs_std::Vec<uint8_t>> resolved_namespace_;
  std::optional<rs_std::Vec<uint8_t>> value_;
};

class QuickReader::Attributes {
 public:
  // Returns the next attribute of the current start-tag, or std::nullopt once
  // the attribute list is exhausted.
  //
  // Lifetime: the returned Attribute holds a non-owning pointer to the
  // QuickReader's heap-allocated parser state (used by ResolvedNamespace()),
  // and its accessors hand out views into cached buffers owned by the
  // Attribute itself. The QuickReader must outlive the Attribute, and the
  // Attribute (or the instance it was moved into) must outlive any view taken
  // from it.
  //
  // KNOWN GAP: the return type routes through std::optional, and
  // `std::optional::operator->` is the one optional accessor that is neither
  // ref-qualified nor ABSL_ATTRIBUTE_LIFETIME_BOUND. It therefore breaks the
  // lifetime chain, and this compiles *silently* and dangles at runtime:
  //     absl::string_view v = (*attrs.NextAttribute())->Value();
  // Fully dereferencing instead reaches the deleted `&&` overload and IS
  // rejected, but it is an unpleasant spelling:
  //     absl::string_view v = (**attrs.NextAttribute()).Value();  // error
  // Prefer binding the optional to a named local:
  //     ASSIGN_OR_RETURN(std::optional<Attribute> attr,
  //                      attrs.NextAttribute());
  //     if (attr.has_value()) { absl::string_view v = attr->Value(); ... }
  absl::StatusOr<std::optional<Attribute>> NextAttribute()
      ABSL_ATTRIBUTE_LIFETIME_BOUND;

 private:
  friend class QuickReader;

  // `reader` is not owned by the constructed object and must outlive it.
  Attributes(rust::Attributes attributes,
             rust::QuickReader* absl_nonnull reader
                 ABSL_ATTRIBUTE_LIFETIME_BOUND)
      : reader_(ABSL_DIE_IF_NULL(reader)), attributes_(std::move(attributes)) {}

  rust::QuickReader* reader_;  // Not owned.
  rust::Attributes attributes_;
};

struct QuickReader::Start {
  QuickReader::Name name;
  QuickReader::Attributes attributes;
  // The substring of the input XML that is the subtree of the tag, excluding
  // and including, resp., the tag itself. Filled if and only if the
  // `xml_substrings` option was enabled.
  //
  // Lifetime differs from other views in this header: these point into the
  // buffer the caller passed to Create(), not into reader state, so they are
  // NOT invalidated by the next NextEvent(). They are valid for exactly as
  // long as that input buffer is. Being plain data members, they carry no
  // ref-qualification or LIFETIME_BOUND guard at all; nothing here is checked
  // at compile time.
  std::optional<absl::string_view> inner_xml;
  std::optional<absl::string_view> outer_xml;
  bool self_closing;
};

struct QuickReader::End {
  QuickReader::Name name;
};

struct QuickReader::Text {
  QuickReader::Data data;
};

struct QuickReader::CData {
  QuickReader::Data data;
};

struct QuickReader::Comment {
  QuickReader::Data data;
};

struct QuickReader::Decl {
  QuickReader::Data data;
};

struct QuickReader::PI {
  QuickReader::Data data;
};

struct QuickReader::DocType {
  QuickReader::Data data;
};

// Represents an XML entity reference (e.g. "&lt;").
struct QuickReader::GeneralRef {
  QuickReader::Data data;
  // The string expanded from the entity reference (e.g. "<"). Works also for
  // numeric entities ("&#...;"), but does not work for user-defined ones, to
  // prevent XXE attacks. Empty if the entity could not be parsed.
  std::string expanded;
};

// Returns the encoding of the XML document, if it is declared. `xml` must be in
// an ASCII-compatible encoding (so, e.g., UTF-16 will not work).
std::optional<std::string> GetXmlEncoding(absl::string_view xml);

// Provides a human-readable description of the event for debugging/logging. It
// may call some methods on `event` that may trigger string copies of the
// associated portions of the XML document. Those copies are cached, which is
// why `event` needs to be a mutable reference.
std::string DescribeEvent(QuickReader::Event& event);

}  // namespace security::xml

#endif  // SECURITY_XML_QUICK_READER_H_
