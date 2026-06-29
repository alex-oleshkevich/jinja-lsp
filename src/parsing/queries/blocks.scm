; Captures {% block name %}, {% block name scoped %}, {% block name required %}.
; Note: "scoped" detection falls back to text scan in the extractor because
; the grammar does not expose a scoped_keyword node type.
(block_statement
  (identifier) @name)

(block_statement
  (identifier) @name
  "required" @required)
