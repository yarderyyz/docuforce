((line_comment
  doc: (doc_comment) @comment)*
  .
  (function_item name: (identifier) @code.identifier) @code.body)
