---
name: "title"
category: "filter"
signature: "title()"
since: "2.0"
params: []
---

Convert a string to title case, capitalizing the first letter of each word. This uses Python's `str.title()` method internally, which means characters following non-letter characters are also capitalized. Use this for formatting headings or display names.

## Usage

```jinja
{{ "hello world" | title }}
{{ article.title | title }}
```
