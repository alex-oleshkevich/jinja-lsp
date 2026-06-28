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
