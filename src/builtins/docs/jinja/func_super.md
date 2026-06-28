---
name: "super"
category: "function"
signature: "super()"
since: "2.0"
params: []
---

Call the parent block's content from within a child template's block override. Renders the content that the parent template defined for that block, allowing the child to extend rather than fully replace it.

## Usage

```jinja
{% block content %}{{ super() }} extra{% endblock %}
```
