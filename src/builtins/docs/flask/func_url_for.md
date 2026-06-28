---
name: "url_for"
category: "function"
signature: "url_for(endpoint, **values)"
since: "0.1"
params:
  - name: "endpoint"
    type: "string"
    required: true
  - name: "values"
    type: "kwargs"
    required: false
---

Generates a URL for the given Flask route endpoint by name. Pass keyword arguments matching the route's variable parts; any extra keyword arguments are appended as query string parameters. Use `_external=True` to get an absolute URL including the scheme and host.

## Usage

```jinja
{# Link to a named route #}
<a href="{{ url_for('index') }}">Home</a>

{# Route with a variable segment #}
<a href="{{ url_for('user_profile', username=user.name) }}">Profile</a>

{# Absolute URL with extra query params #}
<a href="{{ url_for('search', q='jinja', _external=True) }}">Search</a>
```
