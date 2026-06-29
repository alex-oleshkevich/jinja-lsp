; Captures {% from "path" import … %} → FromImport + TemplateReference(From).
; Matches import_statement WITH import_from (has "from" prefix).
(import_statement
  (import_from
    (string_literal) @source)
  (identifier) @name)
