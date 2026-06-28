---
name: "number"
category: "filter"
signature: "number(value, locale=None)"
since: "0.1"
params:
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats an integer or float as a locale-aware number string using Babel, applying the correct thousands separator and decimal symbol for the target locale. When `locale` is `None`, the active locale from the current request context is applied automatically. Useful for displaying counts, scores, or any plain numeric value that should respect regional formatting conventions.

## Usage

```jinja
{# 1,234,567 in en_US or 1.234.567 in de_DE #}
{{ product.stock | number }}

{# Explicit locale #}
{{ stats.total_views | number(locale='fr_FR') }}
```
