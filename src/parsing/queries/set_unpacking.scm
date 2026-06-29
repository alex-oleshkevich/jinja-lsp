; Captures {% set a, b = value %} → multiple VariableDefinitions.
; Same node type as set; the commaSep1 allows multiple expressions before '='.
; Use a broad capture and filter in the extraction layer (multiple @name captures
; in one match indicate tuple unpacking).
(set_statement
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @name))))
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @name2))))
  (binary_operator)
  (expression) @value)
