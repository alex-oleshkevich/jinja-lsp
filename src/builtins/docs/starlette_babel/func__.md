---
name: "_"
category: "function"
signature: "_(message)"
since: "0.1"
params:
  - name: "message"
    type: "string"
    required: true
---

Translates a message string into the current request locale using gettext. This is the standard `_()` shorthand for `gettext()` and looks up the translation in the active message catalog. If no translation is found, the original `message` string is returned unchanged.

## Usage

```jinja
{# Simple string translation #}
<h1>{{ _("Welcome") }}</h1>

{# Used inline with other output #}
<p>{{ _("Hello, world!") }}</p>

{# Inside a block #}
<button type="submit">{{ _("Save changes") }}</button>
```
