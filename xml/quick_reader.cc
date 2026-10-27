#include "quick_reader.h"

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <variant>

#include "crubit/support/rs_std/result.h"
#include "crubit/support/rs_std/str_ref.h"
#include "crubit/support/rs_std/vec.h"
#include "crubit_helpers/string_conversions.h"
#include "crubit/rust.h"
#include "absl/base/attributes.h"
#include "absl/functional/function_ref.h"
#include "absl/functional/overload.h"
#include "absl/log/log.h"
#include "absl/status/status.h"
#include "absl/status/status_macros.h"
#include "absl/status/statusor.h"
#include "absl/strings/str_cat.h"
#include "absl/strings/string_view.h"

namespace security::xml {

namespace {

using ::security::crubit_helpers::StringViewFromVecU8;

rs_std::Vec<uint8_t>& ReturnCached(
    std::optional<rs_std::Vec<uint8_t>>& cache,
    absl::FunctionRef<rs_std::Vec<uint8_t>()> get) {
  if (!cache.has_value()) {
    cache = get();
  }
  return *cache;
}

// The outer optional is the cache, the inner optional is the semantic of the
// value.
std::optional<absl::string_view> ReturnCachedOptional(
    std::optional<std::optional<rs_std::Vec<uint8_t>>>& cache,
    absl::FunctionRef<std::optional<rs_std::Vec<uint8_t>>()> get) {
  if (!cache.has_value()) {
    cache = get();
  }
  if ((*cache).has_value()) {
    return StringViewFromVecU8(**cache);
  }
  return std::nullopt;
}

absl::Status XmlErrorToStatus(rust::XmlError err) {
  std::string msg(StringViewFromVecU8(err.message));
  switch (err.code.tag) {
    case rust::XmlErrorCode::Tag::InvalidArgument:
      return absl::InvalidArgumentError(msg);
    case rust::XmlErrorCode::Tag::FailedPrecondition:
      return absl::FailedPreconditionError(msg);
    case rust::XmlErrorCode::Tag::Internal:
      return absl::InternalError(msg);
  }
  return absl::InternalError(msg);
}

absl::StatusOr<absl::string_view> ResolveNamespace(
    const rs_std::Vec<uint8_t>& qualified_name,
    std::optional<rs_std::Vec<uint8_t>>& cache,
    absl::FunctionRef<
        rs_std::Result<rust::ResolvedName, rust::XmlError>(
            const rs_std::Vec<uint8_t>&)>
        resolver) {
  if (!cache.has_value()) {
    rs_std::Result<rust::ResolvedName, rust::XmlError>
        resolved_result = resolver(qualified_name);
    if (!resolved_result.has_value()) {
      return XmlErrorToStatus(std::move(resolved_result).err());
    }

    cache = std::move(std::move(resolved_result).value().namespace_uri);
  }
  return StringViewFromVecU8(*cache);
}

struct Substrings {
  absl::string_view inner;
  absl::string_view outer;
};

absl::StatusOr<Substrings> RangeToString(rust::Range range,
                                         absl::string_view base) {
  if (range.outer_begin >= base.size()) {
    return absl::FailedPreconditionError(
        absl::StrCat("Range start ", range.outer_begin,
                     " is out of bounds for base of size ", base.size()));
  }
  if (range.outer_end > base.size()) {
    return absl::FailedPreconditionError(
        absl::StrCat("Range end ", range.outer_end,
                     " is out of bounds for base of size ", base.size()));
  }
  if (range.outer_begin > range.outer_end) {
    return absl::FailedPreconditionError(absl::StrCat(
        "Range start ", range.outer_begin, " > end ", range.outer_end));
  }
  return Substrings{.inner = base.substr(range.inner_begin,
                                         range.inner_end - range.inner_begin),
                    .outer = base.substr(range.outer_begin,
                                         range.outer_end - range.outer_begin)};
}

absl::StatusOr<rs_std::StrRef> ToStrRef(absl::string_view xml) {
  std::optional<rs_std::StrRef> str_ref = rs_std::StrRef::FromUtf8(xml);
  if (!str_ref.has_value()) {
    return absl::InvalidArgumentError("Input string is not UTF-8");
  }
  return *str_ref;
}

rust::ReaderConfig ToRustConfig(const QuickReader::Config& config) {
  rust::ReaderConfig result;
  // LINT.IfChange(ReaderConfig)
  result.xml_substrings = config.xml_substrings;
  result.ignore_namespaces = config.ignore_namespaces;
  result.expand_empty_elements = config.expand_empty_elements;
  result.yield_xmlns_attributes = config.yield_xmlns_attributes;
  // LINT.ThenChange(
  //     rust/quick_reader.rs:ReaderConfig,
  //     quick_reader.h:ReaderConfig
  // )
  return result;
}

}  // namespace

absl::string_view QuickReader::Data::Content() & ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(content_, [this]() { return event_.content(); }));
}

absl::string_view QuickReader::Name::QualifiedName() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(qualified_name_, [this]() { return event_.qname(); }));
}

absl::string_view QuickReader::Name::LocalName() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(local_name_, [this]() { return event_.local_name(); }));
}

std::optional<absl::string_view> QuickReader::Name::NamespacePrefix() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return ReturnCachedOptional(namespace_prefix_,
                              [this]() { return event_.namespace_prefix(); });
}

absl::StatusOr<absl::string_view> QuickReader::Name::ResolvedNamespace() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return ResolveNamespace(
      ReturnCached(qualified_name_, [this]() { return event_.qname(); }),
      resolved_namespace_, [this](const rs_std::Vec<uint8_t>& qualified_name) {
        return reader_->resolve_tag(qualified_name);
      });
}

bool QuickReader::Name::LocalNameValid() const {
  return event_.local_name_valid;
}

bool QuickReader::Name::PrefixValid() const { return event_.prefix_valid; }

absl::StatusOr<QuickReader> QuickReader::Create(
    absl::string_view xml ABSL_ATTRIBUTE_LIFETIME_BOUND) {
  ASSIGN_OR_RETURN(rs_std::StrRef str_ref, ToStrRef(xml));
  return QuickReader(
      xml, std::make_unique<rust::QuickReader>(
               rust::QuickReader::create(std::move(str_ref))));
}

absl::StatusOr<QuickReader> QuickReader::CreateWithConfig(
    absl::string_view xml ABSL_ATTRIBUTE_LIFETIME_BOUND, Config config) {
  ASSIGN_OR_RETURN(rs_std::StrRef str_ref, ToStrRef(xml));
  return QuickReader(xml, std::make_unique<rust::QuickReader>(
                              rust::QuickReader::create_with_config(
                                  std::move(str_ref), ToRustConfig(config))));
}

absl::StatusOr<QuickReader::Event> QuickReader::NextEvent()
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  rs_std::Result<rust::XmlEvent, rust::XmlError> result =
      reader_->next_event();
  if (!result.has_value()) {
    return XmlErrorToStatus(std::move(result).err());
  }
  rust::XmlEvent event = std::move(result).value();
  switch (event.event_type().tag) {
    case rust::EventType::Tag::Empty:
    case rust::EventType::Tag::Start: {
      rs_std::Result<rs_std::Option<rust::Range>,
                     rust::XmlError>
          range_result = reader_->subtree_range();
      if (!range_result.has_value()) {
        absl::Status status = XmlErrorToStatus(std::move(range_result).err());
        return absl::Status(
            status.code(), absl::StrCat("Range is not ok: ", status.message()));
      }
      std::optional<rust::Range> opt_range =
          std::move(range_result).value();
      std::optional<absl::string_view> inner_xml;
      std::optional<absl::string_view> outer_xml;
      if (opt_range.has_value()) {
        ASSIGN_OR_RETURN(Substrings substrings,
                         RangeToString(std::move(*opt_range), xml_));
        inner_xml = substrings.inner;
        outer_xml = substrings.outer;
      }
      rust::Attributes attrs = event.attrs();
      bool self_closing =
          event.event_type().tag == rust::EventType::Tag::Empty;
      auto result =
          Start{.name = Name(std::move(event), reader_.get()),
                .attributes = Attributes(std::move(attrs), reader_.get()),
                .inner_xml = inner_xml,
                .outer_xml = outer_xml,
                .self_closing = self_closing};
      return result;
    }
    case rust::EventType::Tag::End:
      return End{.name = Name(std::move(event), reader_.get())};
    case rust::EventType::Tag::Text:
      return Text{.data = Data(std::move(event))};
    case rust::EventType::Tag::CData:
      return CData{.data = Data(std::move(event))};
    case rust::EventType::Tag::Comment:
      return Comment{.data = Data(std::move(event))};
    case rust::EventType::Tag::Decl:
      return Decl{.data = Data(std::move(event))};
    case rust::EventType::Tag::PI:
      return PI{.data = Data(std::move(event))};
    case rust::EventType::Tag::DocType:
      return DocType{.data = Data(std::move(event))};
    case rust::EventType::Tag::GeneralRef: {
      std::string expanded(StringViewFromVecU8(event.expanded_entity));
      Data data(std::move(event));
      return GeneralRef{.data = std::move(data),
                        .expanded = std::move(expanded)};
    }
    case rust::EventType::Tag::Eof:
      return Eof{};
  }
  // If this is reached, the event value is likely invalid and undefined
  // behaviour might be triggering. Following go/logging#severity, this
  // means the need to abort.
  LOG(FATAL) << "Invalid event type: "
             << static_cast<int>(event.event_type().tag);
}

absl::string_view QuickReader::Attribute::QualifiedName() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(qualified_key_, [this]() { return attribute_.key; }));
}

absl::string_view QuickReader::Attribute::LocalName() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(local_name_, [this]() { return attribute_.local_name(); }));
}

std::optional<absl::string_view> QuickReader::Attribute::NamespacePrefix() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return ReturnCachedOptional(
      namespace_prefix_, [this]() { return attribute_.namespace_prefix(); });
}

absl::StatusOr<absl::string_view> QuickReader::Attribute::ResolvedNamespace() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return ResolveNamespace(
      ReturnCached(qualified_key_, [this]() { return attribute_.key; }),
      resolved_namespace_, [this](const rs_std::Vec<uint8_t>& qualified_key) {
        return reader_->resolve_attribute(qualified_key);
      });
}

absl::string_view QuickReader::Attribute::Value() &
    ABSL_ATTRIBUTE_LIFETIME_BOUND {
  return StringViewFromVecU8(
      ReturnCached(value_, [this]() { return attribute_.value; }));
}

bool QuickReader::Attribute::LocalNameValid() const {
  return attribute_.local_name_valid;
}

bool QuickReader::Attribute::PrefixValid() const {
  return attribute_.prefix_valid;
}

absl::StatusOr<std::optional<QuickReader::Attribute>>
QuickReader::Attributes::NextAttribute() ABSL_ATTRIBUTE_LIFETIME_BOUND {
  rs_std::Result<rs_std::Option<rust::Attribute>, rs_std::Vec<uint8_t>>
      attr_result = attributes_.next_attribute();
  if (!attr_result.has_value()) {
    return absl::InternalError(
        absl::StrCat("Failed to read attribute: ",
                     StringViewFromVecU8(std::move(attr_result).err())));
  }
  std::optional<rust::Attribute> opt_attr =
      std::move(attr_result).value();
  if (!opt_attr.has_value()) {
    return std::nullopt;
  }
  return Attribute(std::move(*opt_attr), reader_);
}

std::optional<std::string> GetXmlEncoding(absl::string_view xml) {
  std::optional<rs_std::Vec<uint8_t>> encoding =
      rust::get_xml_encoding(xml);
  if (!encoding.has_value()) {
    return std::nullopt;
  }
  return std::string(StringViewFromVecU8(*encoding));
}

std::string DescribeEvent(QuickReader::Event& event) {
  auto visitor = absl::Overload{
      [](const QuickReader::Eof&) -> std::string { return "Eof"; },
      [](QuickReader::Start& start) -> std::string {
        return absl::StrCat("Start(", start.name.QualifiedName(), ")");
      },
      [](QuickReader::DocType& dt) -> std::string {
        return absl::StrCat("DocType(", dt.data.Content(), ")");
      },
      [](QuickReader::Text& t) -> std::string {
        return absl::StrCat("Text(", t.data.Content(), ")");
      },
      [](QuickReader::CData& cdata) -> std::string {
        return absl::StrCat("CData(", cdata.data.Content(), ")");
      },
      [](QuickReader::PI& pi) -> std::string {
        return absl::StrCat("PI(", pi.data.Content(), ")");
      },
      [](QuickReader::Comment& comment) -> std::string {
        return absl::StrCat("Comment(", comment.data.Content(), ")");
      },
      [](QuickReader::GeneralRef& ref) -> std::string {
        return absl::StrCat("GeneralRef(", ref.data.Content(), ")");
      },
      [](QuickReader::Decl& decl) -> std::string {
        return absl::StrCat("Decl(", decl.data.Content(), ")");
      },
      [](QuickReader::End& end) -> std::string {
        return absl::StrCat("End(", end.name.QualifiedName(), ")");
      }};
  return std::visit(visitor, event);
}

}  // namespace security::xml
