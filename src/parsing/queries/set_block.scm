; REQ-EXTR-09: Captures {% set name %}…{% endset %} → VariableDefinition.
;
; tree-sitter parses the block-set opening tag as an ERROR node because
; the grammar's set_statement rule requires `alias('=', $.binary_operator)`.
; The "set" keyword and the name expression are still accessible as children
; of the ERROR node, so we match that structure directly.
(ERROR
  "set"
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @name)))))
