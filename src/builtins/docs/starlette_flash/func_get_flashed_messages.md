---
name: "get_flashed_messages"
category: "function"
signature: "get_flashed_messages(request)"
since: "0.1"
params:
  - name: "request"
    type: "Request"
    required: true
---

Returns all flash messages stored in the current session by starlette-flash, consuming them so they do not appear on subsequent requests. Messages are typically added in a route handler via `flash(request, message, category)`. The returned list contains `FlashMessage` objects with `message` and `category` attributes.

## Usage

```jinja
{# Display all flashed messages #}
{% for flash in get_flashed_messages(request) %}
  <div class="alert alert-{{ flash.category }}">
    {{ flash.message }}
  </div>
{% endfor %}

{# No messages — show nothing #}
{% set messages = get_flashed_messages(request) %}
{% if messages %}
  <ul>
    {% for m in messages %}<li>{{ m.message }}</li>{% endfor %}
  </ul>
{% endif %}
```
