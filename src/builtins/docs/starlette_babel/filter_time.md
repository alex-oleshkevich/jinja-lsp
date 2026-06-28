---
name: "time"
category: "filter"
signature: "time(value, format='medium', locale=None)"
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

Formats a `time` or `datetime` object as a localized time string using Babel. Accepts Babel's named formats (`'short'`, `'medium'`, `'long'`, `'full'`) or a custom Unicode CLDR pattern such as `'HH:mm'`. The active locale from the current request is used when `locale` is not specified.

## Usage

```jinja
{# Medium format (default): 3:45:00 PM #}
{{ event.starts_at | time }}

{# Short 12-hour format: 3:45 PM #}
{{ event.starts_at | time(format='short') }}

{# 24-hour custom pattern #}
{{ event.starts_at | time(format='HH:mm', locale='fr_FR') }}
```
