---
name: "groupby"
category: "filter"
signature: "groupby(attribute, default=Undefined, case_sensitive=False)"
params:
  - name: "attribute"
    type: "string"
    required: true
  - name: "default"
    type: "any"
    default: "Undefined"
    required: false
  - name: "case_sensitive"
    type: "bool"
    default: "False"
    required: false
---

Group a sequence of objects by a common attribute. Returns a list of `(grouper, list)` namedtuples where `grouper` is the unique attribute value and `list` contains all items with that value. Grouping is case-insensitive by default.

## Usage

```jinja
{% for grouper, items in persons | groupby("city") %}
  <h2>{{ grouper }}</h2>
  {% for person in items %}
    <p>{{ person.name }}</p>
  {% endfor %}
{% endfor %}
```

The `default` parameter specifies what value to use for objects where the attribute is missing, so they are included in the output rather than silently dropped.
