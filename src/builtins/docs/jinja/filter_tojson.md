---
name: "tojson"
category: "filter"
signature: "tojson(indent=None)"
since: "2.9"
params:
  - name: "indent"
    type: "integer"
    default: "None"
    required: false
---

Serialize a value to a JSON string. The output is safe to embed directly in HTML `<script>` tags because characters like `<`, `>`, and `&` are escaped. Provide `indent` to produce pretty-printed JSON with the given indentation level.

## Usage

```jinja
<script>var data = {{ my_data | tojson }};</script>
{{ config | tojson(indent=2) }}
```
