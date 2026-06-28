---
name: "date"
category: "filter"
signature: "date(value, format='medium', locale=None)"
since: "0.1"
params:
  - name: "format"
    type: "string"
    default: "'medium'"
    required: false
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats a `date` or `datetime` object as a localized date string using Babel. The `format` parameter accepts Babel's named formats (`'short'`, `'medium'`, `'long'`, `'full'`) or a custom Unicode CLDR pattern such as `'dd/MM/yyyy'`. When `locale` is `None`, the active locale from the current request context is used automatically.

## Usage

```jinja
{# Medium format (default): Jan 12, 2025 #}
{{ article.published_at | date }}

{# Full locale-aware date #}
{{ article.published_at | date(format='full') }}

{# Custom pattern with explicit locale #}
{{ article.published_at | date(format='dd.MM.yyyy', locale='de_DE') }}
```
