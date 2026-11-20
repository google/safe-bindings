# XML

Wrappers for the [quick-xml](https://crates.io/crates/quick-xml) and
[xmltree-ns](https://crates.io/crates/xmltree-ns) crates.

## Parsing Strictness: xmltree vs libxml2

`xmltree` is fundamentally stricter than `libxml2` when it comes to XML
specification validity, particularly regarding namespaces and qualified names.
While `libxml2` permissively parses malformed namespaces without throwing an
error (default behavior), `xmltree` strictly rejects them.

Concrete examples of `xmltree` enforcing stricter validity include:

1.  **Unbound Namespace Prefixes**

    -   **Example:** `<foo:bar/>` *(where `xmlns:foo` is not defined)*
    -   **libxml2:** Parses successfully.
    -   **xmltree:** Returns error (`prefix is unbound`).

2.  **Invalid Qualified Names**

    -   **Example:** `<:foo/>` *(empty prefix)*
    -   **libxml2:** Parses successfully.
    -   **xmltree:** Returns error (`Qualified name is invalid:`).

3.  **Unexpected Tokens inside Qualified Names**

    -   **Example:** `<a:b:c/>` *(multiple colons)*
    -   **libxml2:** Parses successfully.
    -   **xmltree:** Returns error (`Unexpected token inside qualified name:`).

Consumers migrating from `libxml2` to `xmltree` might implicitly depend on this
"lax" behavior (Hyrum's Law). When migrating existing consumers, ensure their
workloads do not rely on `libxml2`'s permissive parsing of these malformed
namespace patterns.

## Entity Declarations

`XmlDocument::Parse()` and `XmlDocument::ParseFragment()` reject documents that
declare entities in their DTD (`<!DOCTYPE r [<!ENTITY e "...">]>`), and return
an `INVALID_ARGUMENT` error. The underlying `xml` crate does not bound the total
size of entity expansion, so a document of a few hundred bytes could otherwise
expand to gigabytes. The predefined entities (`&lt;`, `&amp;`, ...) and
character references (`&#60;`) still work, and so do DTDs without entity
declarations.

The check looks for `<!ENTITY` anywhere in the input, so a document that
contains it in a comment or CDATA section is rejected too. `QuickReader` does
not expand user-defined entities either.
