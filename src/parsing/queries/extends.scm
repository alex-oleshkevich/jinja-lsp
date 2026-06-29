; Captures {% extends "base.html" %} → TemplateReference(Extends).
; Static path (string_literal):
(extends_statement
  (string_literal) @path)

; Dynamic path (identifier expression — is_dynamic = true):
(extends_statement
  (identifier) @dynamic_path)
