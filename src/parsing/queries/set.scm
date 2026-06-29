; Captures {% set name = value %} → VariableDefinition (Template/Block scope).
; The grammar: set_statement = seq('set', commaSep1(expression), '=', expression).
; Single assignment: the first expression is the variable name.
(set_statement
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @name))))
  (binary_operator)
  (expression) @value)
