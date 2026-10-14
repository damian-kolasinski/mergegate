; tree-sitter-markdown's own injections, except that inline content keeps
; its children: the block grammar leaves emphasis and code-span delimiters as
; anonymous children of `inline`, and without them the inline grammar never
; sees the `*` or backtick that opens a span. Helix sets
; `include-unnamed-children` for the same reason; tree-sitter-highlight only
; knows `include-children`.

(fenced_code_block
  (info_string
    (language) @injection.language)
  (code_fence_content) @injection.content)

((html_block) @injection.content (#set! injection.language "html"))

(document . (section . (thematic_break) (_) @injection.content (thematic_break)) (#set! injection.language "yaml"))

([(minus_metadata) (plus_metadata)] @injection.content (#set! injection.language "yml"))

((inline) @injection.content
  (#set! injection.language "markdown_inline")
  (#set! injection.include-children))
