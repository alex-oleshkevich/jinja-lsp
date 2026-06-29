; Captures {% set name %}…{% endset %} → VariableDefinition.
; The opening tag is the same set_statement node as regular set; the extraction
; layer distinguishes block-set by detecting a missing value expression.
(set_statement
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @name)))))
