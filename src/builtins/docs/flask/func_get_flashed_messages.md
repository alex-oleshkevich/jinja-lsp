---
name: "get_flashed_messages"
category: "function"
signature: "get_flashed_messages(with_categories=False, category_filter=())"
since: "0.3"
params:
  - name: "with_categories"
    type: "bool"
    default: "False"
    required: false
  - name: "category_filter"
    type: "tuple"
    default: "()"
    required: false
---

Returns all messages that were flashed with `flask.flash()` during the previous request. Messages are consumed on retrieval and will not appear again. Pass `with_categories=True` to receive `(category, message)` tuples instead of plain strings, and use `category_filter` to retrieve only messages of specific categories.

## Usage

```jinja
{# Simple message list #}
{% for message in get_flashed_messages() %}
  <div class="alert">{{ message }}</div>
{% endfor %}

{# With categories for styled alerts #}
{% for category, message in get_flashed_messages(with_categories=True) %}
  <div class="alert alert-{{ category }}">{{ message }}</div>
{% endfor %}

{# Only error messages #}
{% for message in get_flashed_messages(category_filter=["error"]) %}
  <div class="alert alert-error">{{ message }}</div>
{% endfor %}
```
