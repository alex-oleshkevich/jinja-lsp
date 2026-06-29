; Captures {% call (var) func() %} → VariableDefinition (CallBlock scope).
; Grammar: call_statement = seq('call', optional(seq('(', identifier, ')')), function_call)
; The optional identifier is the caller variable name.
(call_statement
  (identifier) @caller_var
  (function_call))
