(comment) @comment
(string) @string
["\"proto2\"" "\"proto3\""] @string
[(int_lit) (float_lit)] @number
[(true) (false)] @boolean
[(type) (key_type) (message_name) (enum_name) (service_name)] @type
(rpc_name) @function
["syntax" "edition" "package" "option" "import" "service" "rpc" "returns" "message" "enum" "oneof" "repeated" "reserved" "to"] @keyword
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
(field (identifier) @property)
(oneof_field (identifier) @property)
(map_field (identifier) @property)
(enum_field (identifier) @constant)
(escape_sequence) @string.escape
"=" @operator
[";" "," "."] @punctuation.delimiter
