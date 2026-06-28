---
name: "scientific"
category: "filter"
signature: "scientific(value, locale=None)"
since: "0.1"
params:
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats a number in scientific (exponential) notation using Babel, producing a locale-aware string such as `"1.23E4"`. The exponent separator and decimal symbol follow the conventions of the target locale. When `locale` is `None`, the active locale from the current request context is applied.

## Usage

```jinja
{# 12345.6789 → "1.234568E4" #}
{{ measurement.value | scientific }}

{# With explicit locale #}
{{ measurement.value | scientific(locale='de_DE') }}
```
