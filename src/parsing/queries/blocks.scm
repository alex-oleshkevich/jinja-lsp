; Captures {% block name %} and {% block name required %} → BlockDefinition.
; The grammar: block_statement = seq('block', identifier, optional('required'))
; "scoped" is not in this grammar version.
(block_statement
  (identifier) @name)

(block_statement
  (identifier) @name
  "required" @required)
