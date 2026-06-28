---
name: "center"
category: "filter"
signature: "center(width=80)"
params:
  - name: "width"
    type: "int"
    default: "80"
    required: false
---

Center a string within a field of the given width by padding it with spaces on both sides. Useful for generating plaintext reports or fixed-width output.

## Usage

```jinja
{{ "Hello" | center(40) }}
```

If the string is longer than `width`, it is returned unchanged. The default width of 80 matches a standard terminal line length.
