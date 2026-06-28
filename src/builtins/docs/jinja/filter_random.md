---
name: "random"
category: "filter"
signature: "random()"
since: "2.0"
params: []
---

Return a random item from a sequence. Each time the template is rendered, a different item may be selected. Note that because templates are often cached, the result may not change on every request unless the template is re-evaluated.

## Usage

```jinja
{{ ['rock', 'paper', 'scissors'] | random }}
```
