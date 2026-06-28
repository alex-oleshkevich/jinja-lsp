---
name: "_p"
category: "function"
signature: "_p(singular, plural, n)"
since: "0.1"
params:
  - name: "singular"
    type: "string"
    required: true
  - name: "plural"
    type: "string"
    required: true
  - name: "n"
    type: "int"
    required: true
---

Translates a message to the correct plural form based on the count `n`, using gettext plural rules for the current request locale. Pass the singular and plural English strings; the appropriate translation from the active message catalog is selected according to the locale's pluralization rules. If no translation is found, `singular` is returned when `n == 1`, otherwise `plural`.

## Usage

```jinja
{# "1 item" or "3 items" #}
<p>{{ _p("%(n)s item", "%(n)s items", cart.count) % {'n': cart.count} }}</p>

{# Simple noun without interpolation #}
<p>{{ _p("One result found", "%(n)s results found", total) % {'n': total} }}</p>
```
