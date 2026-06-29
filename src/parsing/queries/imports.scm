; Captures {% import "path" as alias %} → ImportAlias + TemplateReference(Import).
; Matches import_statement WITHOUT import_from (no "from" prefix).
(import_statement
  (string_literal) @source
  (import_as
    (identifier) @alias))
