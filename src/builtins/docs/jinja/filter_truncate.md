---
name: "truncate"
category: "filter"
signature: "truncate(length=255, killwords=False, end='...', leeway=None)"
since: "2.0"
params:
  - name: "length"
    type: "integer"
    default: "255"
    required: false
  - name: "killwords"
    type: "boolean"
    default: "False"
    required: false
  - name: "end"
    type: "string"
    default: "'...'"
    required: false
  - name: "leeway"
    type: "integer"
    default: "None"
    required: false
---

Truncate a string to a given maximum length, appending the `end` string (default `...`) when truncation occurs. By default, truncation occurs at word boundaries so words are not split; set `killwords=True` to allow truncation mid-word. The `leeway` parameter allows the string to exceed `length` by that many characters before truncation is applied, avoiding truncation for only slightly over-length strings.

## Usage

```jinja
{{ long_text | truncate(100) }}
{{ title | truncate(50, killwords=True, end=' …') }}
```
