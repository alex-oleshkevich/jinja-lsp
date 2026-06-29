; Captures {% for a, b in items %} → multiple VariableDefinitions (ForLoop).
; commaSep1(identifier) in in_expression can produce multiple identifiers.
(for_statement
  (in_expression
    (identifier) @name
    (identifier) @name2
    (binary_operator)
    (expression) @iterable))
