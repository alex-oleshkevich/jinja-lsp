; Captures {% with x = value %} → VariableDefinition (With scope).
; Grammar: with_statement = seq('with', repeat(assignment_expression))
; assignment_expression = seq(identifier+, '=', expression)
(with_statement
  (assignment_expression
    (identifier) @name
    (binary_operator)
    (expression) @value))
