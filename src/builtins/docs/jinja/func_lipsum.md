---
name: "lipsum"
category: "function"
signature: "lipsum(n=5, html=True, min=20, max=100)"
since: "2.0"
params:
  - name: "n"
    type: "int"
    default: "5"
    required: false
  - name: "html"
    type: "bool"
    default: "true"
    required: false
  - name: "min"
    type: "int"
    default: "20"
    required: false
  - name: "max"
    type: "int"
    default: "100"
    required: false
---

Generate Lorem Ipsum placeholder text. Produces `n` paragraphs, optionally wrapped in HTML `<p>` tags, with each paragraph containing between `min` and `max` words.

## Usage

```jinja
{{ lipsum(3) }}
```
