; Captures identifier / attribute / filter / function / test usage sites → Reference.

; Identifier uses inside render expressions:
(render_expression
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (identifier) @identifier)))))

; Attribute access (expression.expression):
(render_expression
  (expression
    (expression) @object
    (expression
      (binary_expression
        (unary_expression
          (primary_expression
            (identifier) @attribute))))))

; Filter usage — via binary_operator "|":
(render_expression
  (expression
    (binary_expression
      (binary_expression)
      (binary_operator) @pipe
      (unary_expression
        (primary_expression
          (identifier) @filter)))
    (#eq? @pipe "|")))

; Function calls:
(render_expression
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (function_call
            (identifier) @function))))))

; Builtin `is` test (`x is defined`, `x is callable`, etc.) — in any expression context:
(binary_expression
  (binary_operator) @_is_op
  (builtin_test) @builtin_test
  (#eq? @_is_op "is"))

; Custom (user-defined) `is` test (`x is my_test`) — identifier after `is`:
(binary_expression
  (binary_operator) @_is_op2
  (unary_expression
    (primary_expression
      (identifier) @custom_test))
  (#eq? @_is_op2 "is"))
