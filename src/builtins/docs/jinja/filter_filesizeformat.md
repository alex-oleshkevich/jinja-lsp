---
name: "filesizeformat"
category: "filter"
signature: "filesizeformat(binary=False)"
params:
  - name: "binary"
    type: "bool"
    default: "False"
    required: false
---

Format a file size in bytes into a human-readable string such as `1.2 MB` or `4.0 GiB`. When `binary` is `True`, uses powers of 1024 and IEC suffixes (KiB, MiB, GiB, etc.); when `False` (default), uses powers of 1000 and SI suffixes (kB, MB, GB, etc.).

## Usage

```jinja
{{ file.size | filesizeformat }}
{{ file.size | filesizeformat(binary=True) }}
```

This filter is ideal for storage dashboards, file listing pages, and any context where raw byte counts would be hard for users to interpret.
