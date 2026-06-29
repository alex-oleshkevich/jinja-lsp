; Captures {% macro name(…) %} → MacroDefinition.name
; The macro_statement wraps a function_call whose first identifier is the name.
(macro_statement
  (function_call
    (identifier) @name))
