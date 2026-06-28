---
name: "timedelta"
category: "filter"
signature: "timedelta(value, granularity='second', locale=None)"
since: "0.1"
params:
  - name: "granularity"
    type: "string"
    default: "'second'"
    required: false
  - name: "locale"
    type: "string"
    default: "None"
    required: false
---

Formats a `timedelta` object as a human-readable localized duration string using Babel's `format_timedelta`. The `granularity` parameter controls the smallest unit shown and accepts values such as `'second'`, `'minute'`, `'hour'`, `'day'`, or `'month'`. This is ideal for displaying relative durations like "2 hours, 30 minutes".

## Usage

```jinja
{# Default second granularity: "3 hours, 25 minutes, 10 seconds" #}
{{ duration | timedelta }}

{# Only show up to minutes #}
{{ duration | timedelta(granularity='minute') }}

{# Localized output #}
{{ duration | timedelta(granularity='hour', locale='de_DE') }}
```
