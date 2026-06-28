---
name: "datetime"
category: "filter"
signature: "datetime(value, format='medium', locale=None)"
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

Formats a `datetime` object as a localized date-and-time string using Babel. Accepts Babel's named formats (`'short'`, `'medium'`, `'long'`, `'full'`) or a custom Unicode CLDR pattern. The time zone stored on the datetime object is respected; pass a timezone-aware datetime for correct localized output.

## Usage

```jinja
{# Medium format (default): Jan 12, 2025, 3:45:00 PM #}
{{ event.starts_at | datetime }}

{# Short format #}
{{ event.starts_at | datetime(format='short') }}

{# Custom pattern #}
{{ event.starts_at | datetime(format="yyyy-MM-dd HH:mm", locale='en_US') }}
```
