---
name: "config"
category: "variable"
since: "0.10"
---

The Flask application configuration dictionary, exposed directly in templates. It provides read access to all configuration values set via `app.config`, including built-in Flask settings and any application-specific keys. Avoid exposing sensitive keys (such as `SECRET_KEY` or database credentials) in rendered output.

## Usage

```jinja
{# Conditionally show debug info #}
{% if config['DEBUG'] %}
  <p class="debug">Debug mode is ON</p>
{% endif %}

{# Access a custom config value #}
<p>App version: {{ config.get('APP_VERSION', 'unknown') }}</p>

{# Use attribute-style access #}
{% if config.TESTING %}
  <span>Test environment</span>
{% endif %}
```
