---
name: "percent"
category: "filter"
signature: "percent(value, locale=None)"
since: "0.1"
params:
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats a decimal fraction (0.0–1.0) as a localized percentage string using Babel. The value `0.75` becomes `"75%"` in most locales, but the exact symbol placement and decimal separator follow locale conventions. When `locale` is `None`, the active request locale is used automatically.

## Usage

```jinja
{# 0.754 → "75%" (en_US) #}
{{ stats.completion_rate | percent }}

{# With explicit locale: "75 %" (fr_FR uses a space before %) #}
{{ stats.completion_rate | percent(locale='fr_FR') }}
```
