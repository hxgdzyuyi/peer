(comment) @comment
(string_value) @string
[(int_value) (float_value)] @number
(boolean_value) @boolean
(null_value) @constant
(variable) @variable
(named_type (name) @type)
(field (name) @property)
(field_definition (name) @property)
(directive (name) @attribute)
(fragment_name) @function
["query" "mutation" "subscription" "fragment" "on" "schema" "type" "interface" "union" "enum" "input" "scalar" "extend" "implements" "directive" "repeatable"] @keyword
["{" "}" "(" ")" "[" "]"] @punctuation.bracket
[":" "!" "=" "|" "&" "..." "@" "$" (comma)] @punctuation.delimiter
