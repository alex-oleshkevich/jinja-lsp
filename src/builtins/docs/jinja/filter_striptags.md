---
name: "striptags"
category: "filter"
signature: "striptags()"
since: "2.0"
params: []
---

Strip all HTML and XML tags from a string and normalize runs of whitespace to a single space. This is useful for rendering a plain-text preview of HTML content, such as in email subjects or meta descriptions. Note that it does not sanitize the string for security purposes.

## Usage

```jinja
{{ "<p>Hello <b>World</b></p>" | striptags }}
{{ article.body | striptags | truncate(200) }}
```
