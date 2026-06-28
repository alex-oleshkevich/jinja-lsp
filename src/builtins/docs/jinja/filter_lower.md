---
name: "lower"
category: "filter"
signature: "lower()"
params: []
---

Convert all characters in a string to lowercase. This is a straightforward wrapper around Python's `str.lower()` method and is locale-independent for ASCII characters.

## Usage

```jinja
{{ "Hello World" | lower }}
```

Useful for normalising user input, generating URL slugs, or ensuring consistent case in comparisons and display. For full Unicode-aware lowercasing, Python's underlying `str.lower()` handles most common scripts correctly.
