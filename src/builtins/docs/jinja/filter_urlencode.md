---
name: "urlencode"
category: "filter"
signature: "urlencode()"
since: "2.7"
params: []
---

Percent-encode a string or a mapping (dict) for safe use in a URL. When applied to a string, special characters are escaped. When applied to a dict or list of pairs, the result is a URL query string. Slashes are not encoded in strings.

## Usage

```jinja
{{ "hello world & more" | urlencode }}
{{ {'q': 'jinja2', 'page': 1} | urlencode }}
```
