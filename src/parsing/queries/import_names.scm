; Captures imported names and optional aliases from {% from "…" import a, b as c %}.
; Each imported name is an identifier directly under import_statement;
; its optional alias is in import_as.
(import_statement
  (import_from)
  (identifier) @name)

(import_statement
  (import_from)
  (import_as
    (identifier) @alias))
