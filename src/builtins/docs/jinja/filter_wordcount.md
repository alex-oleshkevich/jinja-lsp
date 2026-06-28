---
name: "wordcount"
category: "filter"
signature: "wordcount()"
since: "2.0"
params: []
---

Count the number of words in a string by splitting on whitespace. This is a simple word count that does not account for punctuation attached to words or complex Unicode word boundaries. It is useful for displaying approximate reading length or enforcing limits.

## Usage

```jinja
{{ article.body | wordcount }} words
{% if comment | wordcount > 500 %}Too long!{% endif %}
```
