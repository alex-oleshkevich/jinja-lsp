---
name: "request"
category: "variable"
since: "0.12"
---

The current Starlette `Request` object, automatically injected into templates when using `TemplateResponse`. It provides access to the full incoming HTTP request including the URL, method, query parameters, path parameters, headers, cookies, and the ASGI scope. Use it to read client-supplied data and build conditional template logic.

## Usage

```jinja
{# Display the request method and URL #}
<p>{{ request.method }} {{ request.url }}</p>

{# Read a query parameter (?page=3) #}
<p>Page: {{ request.query_params.get('page', 1) }}</p>

{# Access a path parameter captured from the route #}
<h1>Item: {{ request.path_params['item_id'] }}</h1>

{# Read a cookie #}
{% if request.cookies.get('session') %}
  <p>Session cookie is present</p>
{% endif %}
```
