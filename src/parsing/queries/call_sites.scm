; Captures function call identifiers inside render expressions for E501 wrong-call-args.
; Each @callee match gives the function name; the parent function_call node has arg children.
(render_expression
  (expression
    (binary_expression
      (unary_expression
        (primary_expression
          (function_call
            (identifier) @callee))))))
