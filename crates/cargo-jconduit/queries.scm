;; Pattern Index 0: True Type Aliases (using BodyId = int;)
(alias_declaration
  name: (type_identifier) @alias_name
  type: (_) @alias_target)

;; Pattern Index 1: Standard Primitive/Named Typedefs
(type_definition
  type: [
          (primitive_type)
          (type_identifier)
          (sized_type_specifier)
          (pointer_declarator)
          ] @typedef_target
  declarator: (type_identifier) @typedef_name)

; pattern 2 — struct with explicit attribute block
(declaration
  (attribute_declaration) @struct_decl
  (struct_specifier
    name: (type_identifier) @struct_name
    body: (field_declaration_list) @struct_body))

;; Pattern Index 3: Modern C++ Standalone Structs
(struct_specifier
  name: (type_identifier) @struct_name
  body: (field_declaration_list) @struct_body) @struct_node

;; Pattern Index 4: Enums (Both standalone and typedef style)
[
  (enum_specifier
    name: (type_identifier) @enum_name
    body: (enumerator_list) @enum_body)

  (type_definition
    type: (enum_specifier
            body: (enumerator_list) @enum_body)
    declarator: (type_identifier) @enum_name)
  ]

;; Pattern Index 5: Universal Function & Block Capture
(declaration
  declarator: (function_declarator
                declarator: (_) @fn_name_root
                parameters: (parameter_list) @fn_params)) @fn_decl