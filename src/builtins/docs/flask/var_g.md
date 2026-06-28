---
name: "g"
category: "variable"
since: "0.10"
---

The application context global object, used to store data that multiple functions need during a single request. Unlike `session`, `g` is not persisted between requests — it is reset at the start of every request. It is commonly used to cache database connections, the current authenticated user, or any other per-request state set in `before_request` hooks.

## Usage

```jinja
{# Display the current user set by a before_request hook #}
{% if g.user %}
  <p>Logged in as {{ g.user.name }}</p>
{% else %}
  <p>Not logged in</p>
{% endif %}

{# Access any arbitrary data stored on g #}
{% if g.get('locale') %}
  <p>Locale: {{ g.locale }}</p>
{% endif %}
```
