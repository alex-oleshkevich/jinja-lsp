---
name: "last"
category: "filter"
signature: "last()"
params: []
---

Return the last item of a sequence. Raises an error if the sequence is empty, so ensure the sequence has at least one element before applying this filter.

## Usage

```jinja
{{ my_list | last }}
```

Useful for extracting the most recent entry from a list (such as the latest log entry or most recent comment) without needing explicit index access or Python slice notation.
