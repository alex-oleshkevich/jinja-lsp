; Captures {% include "path" %} and variants → TemplateReference(Include).
; Static string path:
(include_statement
  (string_literal) @path)

; Dynamic identifier path:
(include_statement
  (identifier) @dynamic_path)

; ignore missing attribute:
(include_statement
  (include_attribute
    (attribute_ignore) @ignore_missing))
