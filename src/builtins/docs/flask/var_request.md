---
name: "request"
category: "variable"
since: "0.1"
---

The current HTTP request object, available automatically in Flask templates. It exposes all incoming request data including the URL, method, query parameters, form fields, JSON body, headers, and cookies. This object is bound to the current request context and is only valid during request handling.

## Usage

```jinja
{# Access the request path and method #}
<p>{{ request.method }} {{ request.path }}</p>

{# Read a query parameter (?page=2) #}
<p>Page: {{ request.args.get('page', 1) }}</p>

{# Check a submitted form field #}
{% if request.form.get('username') %}
  <p>Hello, {{ request.form['username'] }}!</p>
{% endif %}

{# Inspect request headers #}
<p>User-Agent: {{ request.headers.get('User-Agent') }}</p>
```
