---
name: "wordwrap"
category: "filter"
signature: "wordwrap(s, width=79, break_long_words=True, wrapstring=None, break_on_hyphens=True)"
since: "2.0"
params:
  - name: "width"
    type: "int"
    default: "79"
    required: false
  - name: "break_long_words"
    type: "bool"
    default: "True"
    required: false
  - name: "wrapstring"
    type: "string"
    default: "None"
    required: false
  - name: "break_on_hyphens"
    type: "bool"
    default: "True"
    required: false
---

Wrap text at a given line length. By default wraps at 79 characters.
`wrapstring` overrides the string used to join wrapped lines (default is newline).

## Usage

```jinja
{{ long_text | wordwrap(60) }}
{{ text | wordwrap(40, wrapstring='<br>\n') }}
```
