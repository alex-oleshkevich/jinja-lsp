; Captures {% for item in items %} → VariableDefinition (ForLoop scope).
; Grammar: for_statement = seq('for', in_expression, …)
; in_expression = seq(commaSep1(identifier), 'in', expression)
; Single loop variable: one identifier before 'in'.
(for_statement
  (in_expression
    (identifier) @name
    (binary_operator)
    (expression) @iterable))
