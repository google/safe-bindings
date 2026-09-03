
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
