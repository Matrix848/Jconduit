use crate::compiler::{Compiler, CompilerError};
use regex::Regex;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;
use std::mem;
use std::mem::Discriminant;
use std::path::Path;
use std::sync::OnceLock;
use thiserror::Error;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};
// ------------------ Parser ------------------

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    #[error("Preprocessing failed")]
    HeaderPreprocessFailed(#[from] CompilerError),
    #[error("Invalid attribute token: '{0}'")]
    InvalidAttributeToken(String),
    #[error("Attribute '{0}' missing required arguments")]
    MissingArguments(String),
    #[error("Invalid out type: '{0}'")]
    InvalidOutType(String),
    #[error("Duplicate attribute: '{0}'")]
    DuplicateAttribute(String),
}

pub fn preprocess_header(header: &Path) -> Result<String, ParseError> {
    let compiler = Compiler::get();
    compiler.verify_header(header)?;

    let output = compiler
        .command()
        .args(["-E", "-P"])
        .arg(header)
        .output()
        .expect("syntax check passed but preprocessing failed");

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub trait Attribute: Debug + Eq + Hash + Clone {
    fn parse_attrs(s: &str) -> Result<Self, ParseError>
    where
        Self: Sized;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributesMap<T: Attribute> {
    map: HashMap<Discriminant<T>, T>,
}

impl<T: Attribute> From<HashMap<Discriminant<T>, T>> for AttributesMap<T> {
    fn from(map: HashMap<Discriminant<T>, T>) -> Self {
        Self { map }
    }
}

impl<T: Attribute> Default for AttributesMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Attribute> AttributesMap<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn has(&self, attr: &T) -> bool {
        self.map.contains_key(&mem::discriminant(attr))
    }

    pub fn get(&self, attr: &T) -> Option<&T> {
        self.map.get(&mem::discriminant(attr))
    }

    pub fn insert(&mut self, attr: T) -> Result<(), ParseError> {
        let key = mem::discriminant(&attr);
        match self.map.entry(key) {
            Entry::Occupied(_) => {
                // We only format the error string if it actually fails
                Err(ParseError::DuplicateAttribute(format!("{:?}", attr)))
            }
            Entry::Vacant(entry) => {
                entry.insert(attr);
                Ok(())
            }
        }
    }
}

pub fn parse_attributes<T: Attribute>(attrs_str: &str) -> Result<AttributesMap<T>, ParseError> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?x) jcd::\w+ (?: \s* \( [^)]* \) )?").unwrap());

    let mut attrs = HashMap::new();

    for mat in re.find_iter(attrs_str) {
        let token = mat.as_str().trim();
        let attr = T::parse_attrs(token)?;
        let key = mem::discriminant(&attr);

        if attrs.insert(key, attr).is_some() {
            return Err(ParseError::DuplicateAttribute(token.to_string()));
        }
    }

    Ok(AttributesMap::from(attrs))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    /// `is_const` here means the named type itself is const (e.g., `const Foo`).
    Named {
        name: String,
        is_const: bool,
    },
    /// `is_const` means the pointer itself is const (e.g., `Foo* const`).
    Pointer {
        to: Box<TypeKind>,
        is_const: bool,
    },
    Reference(Box<TypeKind>),
    FixedArray {
        element_type: Box<TypeKind>,
        size: usize,
    },
    FunctionPointer {
        return_type: Box<TypeKind>,
        param_types: Vec<TypeKind>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeKind,
    pub attributes: AttributesMap<FieldAttributes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldAttributes {
    Count,
}

impl Attribute for FieldAttributes {
    fn parse_attrs(s: &str) -> Result<Self, ParseError> {
        let trimmed = s
            .strip_prefix("jcd::")
            .ok_or_else(|| ParseError::InvalidAttributeToken(s.to_string()))?
            .trim();

        let (attr_name, rest) = match trimmed.split_once('(') {
            Some((name, rest)) => (name.trim(), Some(rest)),
            None => (trimmed, None),
        };

        match attr_name {
            "count" if rest.is_none() => Ok(Self::Count),
            _ => Err(ParseError::InvalidOutType(attr_name.to_string())),
        }
    }
}

impl AttributesMap<FieldAttributes> {
    pub fn is_count(&self) -> bool {
        self.has(&FieldAttributes::Count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub ty: TypeKind,
    pub attributes: AttributesMap<ParameterAttributes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParameterAttributes {
    Out,
}

impl Attribute for ParameterAttributes {
    fn parse_attrs(s: &str) -> Result<Self, ParseError> {
        let trimmed = s
            .strip_prefix("jcd::")
            .ok_or_else(|| ParseError::InvalidAttributeToken(s.to_string()))?
            .trim();

        let (attr_name, rest) = match trimmed.split_once('(') {
            Some((name, rest)) => (name.trim(), Some(rest)),
            None => (trimmed, None),
        };

        match attr_name {
            "out" if rest.is_none() => Ok(Self::Out),
            _ => Err(ParseError::InvalidOutType(attr_name.to_string())),
        }
    }
}

impl AttributesMap<ParameterAttributes> {
    pub fn is_out(&self) -> bool {
        self.has(&ParameterAttributes::Out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FunctionAttributes {
    Direct,
    NoScratchpad,
    OutHandle(HandleType),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HandleType {
    Clone(Option<String>),
    Copy(Option<String>),
    UnsafeView(Option<String>),
    AtomicView(Option<String>),
}

impl Default for HandleType {
    fn default() -> Self {
        Self::Copy(None)
    }
}

impl Attribute for FunctionAttributes {
    fn parse_attrs(s: &str) -> Result<Self, ParseError> {
        let trimmed = s
            .strip_prefix("jcd::")
            .ok_or_else(|| ParseError::InvalidAttributeToken(s.to_string()))?
            .trim();

        let (attr_name, rest) = match trimmed.split_once('(') {
            Some((name, rest)) => (name.trim(), Some(rest)),
            None => (trimmed, None),
        };

        match attr_name {
            "direct" => Ok(Self::Direct),
            "no_scratchpad" => Ok(Self::NoScratchpad),
            "handle" => {
                let inner = rest
                    .ok_or_else(|| ParseError::MissingArguments(s.to_string()))?
                    .strip_suffix(')')
                    .ok_or_else(|| ParseError::InvalidAttributeToken(s.to_string()))?;

                let mut tokens = inner.split(',').map(str::trim).filter(|s| !s.is_empty());

                let strategy = tokens
                    .next()
                    .ok_or_else(|| ParseError::MissingArguments("missing strategy".to_string()))?;

                let custom_fn = tokens.next().map(str::to_string);

                let handle = match strategy {
                    "copy" => HandleType::Copy(custom_fn),
                    "clone" => HandleType::Clone(custom_fn),
                    "unsafe_view" => HandleType::UnsafeView(custom_fn),
                    "atomic_view" => HandleType::AtomicView(custom_fn),
                    _ => return Err(ParseError::InvalidHandleType(strategy.to_string())),
                };

                Ok(Self::OutHandle(handle))
            }
            _ => Err(ParseError::InvalidOutType(attr_name.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AstItem {
    TypeDef {
        name: String,
        target: TypeKind,
    },
    Struct {
        name: String,
        fields: Vec<Field>,
        alignment: Option<usize>,
        is_vec: bool,
    },
    Enum {
        name: String,
        underlying_type: Option<TypeKind>,
        variants: Vec<EnumVariant>,
    },
    Function {
        name: String,
        params: Vec<Parameter>,
        return_type: TypeKind,
        attributes: AttributesMap<FunctionAttributes>,
    },
}

impl AttributesMap<FunctionAttributes> {
    pub fn is_direct(&self) -> bool {
        self.has(&FunctionAttributes::Direct)
    }
    pub fn is_no_scratchpad(&self) -> bool {
        self.has(&FunctionAttributes::NoScratchpad)
    }

    pub fn get_handle_type(&self) -> Option<HandleType> {
        let handle_attr = self.get(&FunctionAttributes::OutHandle(Default::default()))?;
        if let FunctionAttributes::OutHandle(handle_ty) = handle_attr {
            Some(handle_ty.clone())
        } else {
            None
        }
    }
}

impl AstItem {
    fn name(&self) -> &str {
        match self {
            Self::TypeDef { name, .. }
            | Self::Struct { name, .. }
            | Self::Enum { name, .. }
            | Self::Function { name, .. } => name,
        }
    }

    fn is_duplicate_of(&self, other: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(other) && self.name() == other.name()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRegistry {
    pub typedefs: HashMap<String, TypeKind>,
    pub structs: HashMap<String, StructMetadata>,
    pub enums: HashMap<String, EnumMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMetadata {
    pub alignment: Option<usize>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMetadata {
    pub underlying_type: TypeKind,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Default)]
pub struct AbstractSyntaxTree {
    pub items: Vec<AstItem>,
}

impl AbstractSyntaxTree {
    fn already_parsed(&self, item: &AstItem) -> bool {
        self.items.iter().any(|i| i.is_duplicate_of(item))
    }
}

pub struct HeaderParser {
    source: String,
    query: Query,
}

impl HeaderParser {
    pub fn new(source_code: &str, query_dsl: &str) -> Self {
        let query = Query::new(&tree_sitter_cpp::LANGUAGE.into(), query_dsl)
            .expect("invalid tree-sitter query");

        Self {
            source: source_code.to_string(),
            query,
        }
    }

    pub fn parse(&self) -> Result<AbstractSyntaxTree, ParseError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .unwrap();

        let tree = parser.parse(&self.source, None).unwrap();
        let mut cursor = QueryCursor::new();
        cursor.set_match_limit(8192);

        let mut ast = AbstractSyntaxTree::default();
        let source_bytes = self.source.as_bytes();
        let capture_names = self.query.capture_names();

        let cap = |name| self.query.capture_index_for_name(name);
        let alias_idx = cap("alias_target");
        let typedef_idx = cap("typedef_target");
        let struct_decl_idx = cap("struct_decl");
        let struct_body_idx = cap("struct_body");
        let align_val_idx = cap("align_val");
        let enum_base_idx = cap("enum_base");
        let enum_body_idx = cap("enum_body");
        let fn_params_idx = cap("fn_params");
        let fn_name_root_idx = cap("fn_name_root");
        let fn_decl_idx = cap("fn_decl");
        let struct_name_idx = cap("struct_name").unwrap_or(0);
        let enum_name_idx = cap("enum_name").unwrap_or(0);

        let mut matches = cursor.captures(&self.query, tree.root_node(), source_bytes);

        while let Some((mat, _)) = matches.next() {
            let first_cap = &mat.captures[0];
            let tag = capture_names[first_cap.index as usize];

            let node_text = |node| self.node_text(node);
            let first_node = |idx: Option<u32>| -> Option<Node> {
                idx.and_then(|i| mat.nodes_for_capture_index(i).next())
            };

            match mat.pattern_index {
                // type alias / typedef
                0 | 1 => {
                    let target_idx = if tag == "alias_name" {
                        alias_idx
                    } else {
                        typedef_idx
                    };

                    if let Some(target) = first_node(target_idx) {
                        let text = node_text(target);
                        if !text.contains("struct") && !text.contains("enum") {
                            let item = AstItem::TypeDef {
                                name: node_text(first_cap.node),
                                target: self.resolve_type(target, None),
                            };
                            if !ast.already_parsed(&item) {
                                ast.items.push(item);
                            }
                        }
                    }
                }

                // struct
                2 | 3 => {
                    if let (Some(name_node), Some(body)) = (
                        first_node(Some(struct_name_idx)),
                        first_node(struct_body_idx),
                    ) {
                        let alignment = first_node(align_val_idx)
                            .and_then(|n| node_text(n).parse::<usize>().ok());

                        let is_vec = first_node(struct_decl_idx)
                            .and_then(|d| d.child(0))
                            .filter(|c| c.kind() == "attribute_declaration")
                            .map(|c| node_text(c).contains("jcd::vec"))
                            .unwrap_or(false);

                        let item = AstItem::Struct {
                            name: node_text(name_node),
                            fields: self.extract_fields(body)?,
                            alignment,
                            is_vec,
                        };
                        if !ast.already_parsed(&item) {
                            ast.items.push(item);
                        }
                    }
                }

                // enum
                4 => {
                    if let Some(name_node) = first_node(Some(enum_name_idx))
                        && let Some(body) = first_node(enum_body_idx)
                    {
                        let underlying_type =
                            first_node(enum_base_idx).map(|n| self.resolve_type(n, None));

                        let item = AstItem::Enum {
                            name: node_text(name_node),
                            underlying_type,
                            variants: self.extract_enum_variants(body),
                        };
                        if !ast.already_parsed(&item) {
                            ast.items.push(item);
                        }
                    }
                }

                // function
                5 => {
                    let (Some(name_root), Some(params), Some(decl)) = (
                        first_node(fn_name_root_idx),
                        first_node(fn_params_idx),
                        first_node(fn_decl_idx),
                    ) else {
                        continue;
                    };

                    let clean_name = self.peel_declarator(name_root);
                    let fn_name = node_text(clean_name);

                    let Some(type_node) = self.find_declaration_type(name_root) else {
                        continue;
                    };

                    let attributes = match decl.child(0) {
                        Some(first) if first.kind() == "attribute_declaration" => {
                            Some(parse_attributes(node_text(first).as_str())?)
                        }
                        _ => None,
                    };

                    let item = AstItem::Function {
                        name: fn_name,
                        params: self.extract_parameters(params)?,
                        return_type: self.resolve_type(type_node, Some(name_root)),
                        attributes: attributes.unwrap_or_default(),
                    };
                    if !ast.already_parsed(&item) {
                        ast.items.push(item);
                    }
                }

                _ => {}
            }
        }

        pub const SYSTEM_PRIMITIVES: &[&str] = &[
            // Core & Void
            "void",
            "bool",
            // Fixed-Width Signed
            "int8_t",
            "int16_t",
            "int32_t",
            "int64_t",
            // Fixed-Width Unsigned
            "uint8_t",
            "uint16_t",
            "uint32_t",
            "uint64_t",
            // Standard Built-in Signed
            "char",
            "signed char",
            "short",
            "short int",
            "signed short",
            "signed short int",
            "int",
            "signed int",
            "long",
            "long int",
            "signed long",
            "signed long int",
            "long long",
            "long long int",
            "signed long long",
            "signed long long int",
            // Standard Built-in Unsigned
            "unsigned char",
            "unsigned short",
            "unsigned short int",
            "unsigned int",
            "unsigned long",
            "unsigned long int",
            "unsigned long long",
            "unsigned long long int",
            // Floating Point
            "float",
            "double",
            "long double",
            // Memory, Sizes, and Pointers
            "size_t",
            "ssize_t",
            "ptrdiff_t",
            "uintptr_t",
            "intptr_t",
            // Wide & Unicode Characters
            "wchar_t",
            "char8_t",
            "char16_t",
            "char32_t",
        ];

        ast.items.retain(|item| {
            let name = item.name();
            !name.is_empty() && !name.starts_with('_') && !SYSTEM_PRIMITIVES.contains(&name)
        });

        Ok(ast)
    }

    fn resolve_type(&self, type_node: Node, decl_node: Option<Node>) -> TypeKind {
        let text = self.node_text(type_node);
        let base_is_const = text.contains("const");

        let base_name = text
            .replace("const", "")
            .replace("struct", "")
            .replace("enum", "")
            .trim()
            .to_string();

        let mut kind = TypeKind::Named {
            name: base_name,
            is_const: base_is_const,
        };
        let mut current = decl_node;

        while let Some(node) = current {
            match node.kind() {
                "pointer_declarator" => {
                    let ptr_is_const = (0..node.child_count())
                        .filter_map(|i| node.child(i as u32))
                        .any(|c| c.kind() == "type_qualifier" && self.node_text(c) == "const");

                    kind = TypeKind::Pointer {
                        to: Box::new(kind),
                        is_const: ptr_is_const,
                    };
                    current = node.child_by_field_name("declarator");
                }
                "reference_declarator" => {
                    kind = TypeKind::Reference(Box::new(kind));
                    current = node.child_by_field_name("declarator");
                }
                "array_declarator" => {
                    let size = node
                        .child_by_field_name("size")
                        .and_then(|s| self.node_text(s).parse().ok())
                        .unwrap_or(0);

                    kind = TypeKind::FixedArray {
                        element_type: Box::new(kind),
                        size,
                    };
                    current = node.child_by_field_name("declarator");
                }
                "field_identifier" | "identifier" => break,
                _ => {
                    current = node
                        .child_by_field_name("declarator")
                        .or_else(|| node.child(0));
                }
            }
        }

        kind
    }

    fn extract_enum_variants(&self, body: Node) -> Vec<EnumVariant> {
        let mut variants = Vec::new();
        let mut cursor = body.walk();

        if cursor.goto_first_child() {
            loop {
                let n = cursor.node();
                if n.kind() == "enumerator"
                    && let Some(name_node) = n.child_by_field_name("name")
                {
                    let value = n.child_by_field_name("value").map(|v| {
                        let s = self.node_text(v);
                        if s.starts_with("0x") || s.starts_with("0X") {
                            i64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
                        } else {
                            s.parse().unwrap_or(0)
                        }
                    });
                    variants.push(EnumVariant {
                        name: self.node_text(name_node),
                        value,
                    });
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        variants
    }

    fn extract_fields(&self, fields: Node) -> Result<Vec<Field>, ParseError> {
        let mut out = Vec::new();
        let mut cursor = fields.walk();

        for n in fields.children(&mut cursor) {
            if !matches!(n.kind(), "field_declaration" | "declaration") {
                continue;
            }

            let attr_str = self.get_attr_str(n);

            if let (Some(ty), Some(decl)) = (
                n.child_by_field_name("type"),
                n.child_by_field_name("declarator"),
            ) {
                out.push(Field {
                    name: self.node_text(self.peel_declarator(decl)),
                    ty: self.resolve_type(ty, Some(decl)),
                    attributes: parse_attributes(&attr_str)?,
                });
            }
        }

        Ok(out)
    }

    fn extract_parameters(&self, params: Node) -> Result<Vec<Parameter>, ParseError> {
        let mut out = Vec::new();
        let mut cursor = params.walk();

        for n in params.children(&mut cursor) {
            if !matches!(
                n.kind(),
                "parameter_declaration" | "optional_parameter_declaration"
            ) {
                continue;
            }

            let attr_str = self.get_attr_str(n);

            if let (Some(ty), Some(decl)) = (
                n.child_by_field_name("type"),
                n.child_by_field_name("declarator"),
            ) {
                out.push(Parameter {
                    name: self.node_text(self.peel_declarator(decl)),
                    ty: self.resolve_type(ty, Some(decl)),
                    attributes: parse_attributes(&attr_str)?,
                });
            }
        }

        Ok(out)
    }

    fn get_attr_str(&self, node: Node) -> String {
        (0..node.child_count())
            .filter_map(|i| node.child(i as u32))
            .filter(|c| c.kind().contains("attribute"))
            .map(|c| self.node_text(c))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn peel_declarator<'a>(&self, mut node: Node<'a>) -> Node<'a> {
        while let "pointer_declarator"
        | "reference_declarator"
        | "array_declarator"
        | "attributed_declarator" = node.kind()
        {
            node = node
                .child_by_field_name("declarator")
                .or_else(|| node.child(0))
                .unwrap_or(node);
        }
        node
    }

    fn find_declaration_type<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut current = node.parent();
        while let Some(parent) = current {
            if matches!(parent.kind(), "declaration" | "field_declaration") {
                return parent.child_by_field_name("type");
            }
            current = parent.parent();
        }
        None
    }

    #[inline]
    fn node_text(&self, node: Node) -> String {
        node.utf8_text(self.source.as_bytes())
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}
