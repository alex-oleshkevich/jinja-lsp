---
name: "currency"
category: "filter"
signature: "currency(value, currency, locale=None)"
since: "0.1"
params:
  - name: "currency"
    type: "string"
    required: true
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats a numeric value as a localized currency string using Babel, including the correct currency symbol, thousands separator, and decimal places for the given locale. The `currency` argument must be a three-letter ISO 4217 currency code such as `'USD'`, `'EUR'`, or `'GBP'`. When `locale` is `None`, the active locale from the request context is used.

## Usage

```jinja
{# $1,234.56 for en_US #}
{{ order.total | currency('USD') }}

{# 1.234,56 € for de_DE #}
{{ order.total | currency('EUR', locale='de_DE') }}

{# £99.99 #}
{{ product.price | currency('GBP', locale='en_GB') }}
```
