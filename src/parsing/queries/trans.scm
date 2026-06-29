; Captures {% trans count=value %} → VariableDefinition (Trans scope).
; Grammar: trans_statement = seq('trans', commaSep(identifier | assignment_expression))
(trans_statement
  (assignment_expression
    (identifier) @name
    (binary_operator)
    (expression) @value))

; Plain identifier in trans (e.g. {% trans user %}):
(trans_statement
  (identifier) @name)
