---
name: "url_for"
category: "function"
signature: "url_for(name, **path_params)"
since: "0.12"
params:
  - name: "name"
    type: "string"
    required: true
  - name: "path_params"
    type: "kwargs"
    required: false
---

Generates a URL for a named Starlette route, resolving path parameters into the URL pattern. The route name corresponds to the `name` argument passed when registering routes in your Starlette application. Extra keyword arguments not part of the path pattern are appended as query string parameters.

## Usage

```jinja
{# Link to a named route #}
<a href="{{ url_for('homepage') }}">Home</a>

{# Route with a path parameter #}
<a href="{{ url_for('user_detail', username='alice') }}">Alice's profile</a>

{# Static file URL #}
<link rel="stylesheet" href="{{ url_for('static', path='/css/main.css') }}">
```
