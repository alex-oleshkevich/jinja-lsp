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

; Filter after attribute access: {{ obj.attr | filter }}
; The outer expression has two child expressions: the attribute chain and the filter part.
(render_expression
  (expression
    (expression)
    (expression
      (binary_expression
        (binary_expression)
        (binary_operator) @_attr_pipe
        (unary_expression
          (primary_expression
            (identifier) @filter)))
      (#eq? @_attr_pipe "|"))))

; Function calls, anywhere an expression can appear -- render expressions,
; control-statement conditions/iterables ({% if %}/{% for %}), filter args,
; nested call arguments, and method-chain links ({{ f(x).g(y) }}, where the
; attribute-access-specific patterns above/below don't apply since neither
; side is a bare identifier or a pipe-filter). A single unscoped pattern
; matches a function_call at any depth, subsuming what used to be two
; narrower, chain-shape-specific patterns (top-level call, and call-after-
; attribute-access-via-pipe) that both missed direct method chains.
(function_call
  (identifier) @function)

; Inline gettext shorthand: {{ _('message') }}. The grammar parses this as its
; own inline_trans node (seq('_', '(', expression, ')')), not a function_call,
; so `_` is an anonymous literal token here rather than an `identifier` child —
; match it by its literal text like a keyword capture.
(inline_trans "_" @function)

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
