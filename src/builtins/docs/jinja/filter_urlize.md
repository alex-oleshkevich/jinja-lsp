---
name: "urlize"
category: "filter"
signature: "urlize(trim_url_limit=None, nofollow=False, target=None, rel=None, extra_schemes=None)"
since: "2.0"
params:
  - name: "trim_url_limit"
    type: "integer"
    default: "None"
    required: false
  - name: "nofollow"
    type: "boolean"
    default: "False"
    required: false
  - name: "target"
    type: "string"
    default: "None"
    required: false
  - name: "rel"
    type: "string"
    default: "None"
    required: false
  - name: "extra_schemes"
    type: "list"
    default: "None"
    required: false
---

Convert plain-text URLs in a string into clickable HTML anchor tags. Use `trim_url_limit` to shorten the visible link text, `nofollow` to add `rel="nofollow"`, and `target` to control the link target (e.g. `'_blank'`). The `extra_schemes` parameter allows additional URL schemes beyond `http` and `https` to be linked.

## Usage

```jinja
{{ "Visit https://example.com for more." | urlize }}
{{ comment | urlize(trim_url_limit=40, nofollow=True, target='_blank') }}
```
