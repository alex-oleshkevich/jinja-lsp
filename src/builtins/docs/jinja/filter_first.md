---
name: "first"
category: "filter"
signature: "first()"
params: []
---

Return the first item of a sequence. Raises an error if the sequence is empty, so ensure the sequence has at least one element before applying this filter.

## Usage

```jinja
{{ my_list | first }}
```

Commonly used when you need only the leading element of a list without slicing or indexing explicitly, keeping templates concise and readable.
