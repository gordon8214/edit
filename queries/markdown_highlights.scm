; Minimal Markdown highlighting query
; Based on common markdown syntax elements

; Headings
[
  (atx_heading)
  (setext_heading)
] @markup.heading

; Emphasis
(emphasis) @markup.italic
(strong_emphasis) @markup.bold

; Code
(code_span) @markup.raw.inline
(fenced_code_block) @markup.raw.block
(indented_code_block) @markup.raw.block

; Links
(link_destination) @markup.link.url
(link_label) @markup.link.label
(link_text) @markup.link.text

; Lists
(list_marker) @punctuation.special

; Quotes
(block_quote) @markup.quote

; Thematic break
(thematic_break) @punctuation.special
