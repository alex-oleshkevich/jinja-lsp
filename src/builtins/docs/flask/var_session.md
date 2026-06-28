---
name: "session"
category: "variable"
since: "0.1"
---

The current user session, implemented as a dictionary that persists data between requests using a client-side signed cookie. Values stored in `session` survive page navigation and browser reloads for the same user. The session is cryptographically signed using the app's `SECRET_KEY`, so clients cannot tamper with its contents.

## Usage

```jinja
{# Check if a user is logged in #}
{% if session.get('user_id') %}
  <p>Welcome back, {{ session['username'] }}!</p>
{% else %}
  <a href="{{ url_for('login') }}">Log in</a>
{% endif %}

{# Display a stored preference #}
<p>Theme: {{ session.get('theme', 'light') }}</p>
```
