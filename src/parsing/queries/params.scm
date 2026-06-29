; Captures macro parameter names and optional defaults → Parameter.
; Positional param: (arg (expression …)) — name is the identifier inside expression.
; Keyword param:    (arg (identifier) @name (binary_operator) (expression …)) — has a default.
(macro_statement
  (function_call
    (arg
      (identifier) @name
      (binary_operator)
      (expression) @default)))

(macro_statement
  (function_call
    (arg
      (expression
        (binary_expression
          (unary_expression
            (primary_expression
              (identifier) @name)))))))
