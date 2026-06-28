---
name: "safe"
category: "filter"
signature: "safe()"
since: "2.0"
params: []
---

Mark a string as safe HTML so that it will not be escaped when rendered in an auto-escaping environment. Use this only when you are certain the value does not contain untrusted user input, as bypassing escaping can introduce XSS vulnerabilities.

## Usage

```jinja
{{ "<strong>bold</strong>" | safe }}
{{ trusted_html_content | safe }}
```
