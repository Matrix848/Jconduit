use crate::parser::{AbstractSyntaxTree, AstItem, TypeKind};
use petgraph::Direction;
use petgraph::algo::toposort;
use petgraph::prelude::DiGraphMap;
use std::collections::HashMap;
use std::fmt::Display;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq, Hash)]
pub enum IrError {
    #[error("Unknown type: {0}.")]
    UnknownType(String),
    #[error("Unsupported type: {0}.")]
    UnsupportedType(String),
    #[error("Enum: {0} uses an unsupported underlying type.")]
    UnsupportedEnumUnderlyingType(String),
    #[error("Only void functions can be deferred")]
    NonVoidDeferredFunction(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    SizeT,
    SsizeT,
    PtrDiffT,

    Void,
    Bool,
    Float,
    Double,
    Uptr,
    Iptr,
    Wchar,
}

impl Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            PrimitiveType::Void => "void".to_string(),
            PrimitiveType::Bool => "bool".to_string(),
            PrimitiveType::I8 => "i8".to_string(),
            PrimitiveType::U8 => "u8".to_string(),
            PrimitiveType::I16 => "i16".to_string(),
            PrimitiveType::U16 => "u16".to_string(),
            PrimitiveType::I32 => "i32".to_string(),
            PrimitiveType::U32 => "u32".to_string(),
            PrimitiveType::I64 => "i64".to_string(),
            PrimitiveType::U64 => "u64".to_string(),
            PrimitiveType::Float => "float".to_string(),
            PrimitiveType::Double => "double".to_string(),
            PrimitiveType::SizeT => "size_t".to_string(),
            PrimitiveType::SsizeT => "ssize_t".to_string(),
            PrimitiveType::PtrDiffT => "ptrdiff_t".to_string(),
            PrimitiveType::Uptr => "uintptr_t".to_string(),
            PrimitiveType::Iptr => "intptr_t".to_string(),
            PrimitiveType::Wchar => "wchar_t".to_string(),
        };
        write!(f, "{}", str)
    }
}

impl PrimitiveType {
    fn from(name: &str) -> Option<PrimitiveType> {
        match name {
            // Void & Boolean
            "void" => Some(PrimitiveType::Void),
            "bool" => Some(PrimitiveType::Bool),
            // 8-Bit Integers
            "int8_t" | "char" | "signedchar" => Some(PrimitiveType::I8),
            "uint8_t" | "unsignedchar" | "char8_t" => Some(PrimitiveType::U8),
            // 16-Bit Integers
            "int16_t" | "short" | "shortint" | "signedshort" | "signedshortint" => {
                Some(PrimitiveType::I16)
            }
            "uint16_t" | "unsignedshort" | "unsignedshortint" | "char16_t" => {
                Some(PrimitiveType::U16)
            }
            // 32-Bit Integers
            "int32_t" | "int" | "signedint" => Some(PrimitiveType::I32),
            "uint32_t" | "unsignedint" | "char32_t" => Some(PrimitiveType::U32),
            // 64-Bit Integers
            "int64_t" | "longlong" | "longlongint" | "signedlonglong" | "signedlonglongint" => {
                Some(PrimitiveType::I64)
            }
            "uint64_t" | "unsignedlonglong" | "unsignedlonglongint" => Some(PrimitiveType::U64),
            // Platform-dependent Arch Ints
            "long" | "longint" | "signedlong" | "signedlongint" => Some(PrimitiveType::I64),
            "unsignedlong" | "unsignedlongint" => Some(PrimitiveType::U64),
            // Floating Point
            "float" => Some(PrimitiveType::Float),
            "double" | "longdouble" => Some(PrimitiveType::Double),
            // Size & Pointer Types
            "size_t" => Some(PrimitiveType::SizeT),
            "ssize_t" => Some(PrimitiveType::SsizeT),
            "ptrdiff_t" => Some(PrimitiveType::PtrDiffT),
            "uintptr_t" => Some(PrimitiveType::Uptr),
            "intptr_t" => Some(PrimitiveType::Iptr),
            // OS Specific Wide Strings
            "wchar_t" => Some(PrimitiveType::Wchar),
            _ => None,
        }
    }

    pub fn byte_size(&self) -> u64 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::Float => 4,
            Self::I64 | Self::U64 | Self::Double => 8,
            Self::SizeT | Self::SsizeT | Self::PtrDiffT | Self::Uptr | Self::Iptr => 8,
            Self::Bool => 1,
            Self::Void => 0,
            Self::Wchar => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IrTypeKind {
    Primitive(PrimitiveType),
    Named(String),
    Pointer {
        to: Box<IrType>,
        is_ptr_const: bool,
    },
    Reference {
        to: Box<IrType>,
    },
    FixedArray {
        element_type: Box<IrType>,
        size: u64,
    },
    FunctionPointer {
        return_type: Box<IrType>,
        arguments: Vec<IrType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IrType {
    pub kind: IrTypeKind,
    pub is_base_const: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrTypedVar {
    pub name: String,
    pub ty: IrType,
}

impl From<IrParameter> for IrTypedVar {
    fn from(param: IrParameter) -> IrTypedVar {
        IrTypedVar {
            name: param.name,
            ty: param.ty,
        }
    }
}

impl From<IrField> for IrTypedVar {
    fn from(field: IrField) -> IrTypedVar {
        IrTypedVar {
            name: field.name,
            ty: field.ty,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrField {
    pub name: String,
    pub ty: IrType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParameter {
    pub name: String,
    pub ty: IrType,
    pub is_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParameter>,
    pub return_type: IrType,
    pub is_deferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrStruct {
    pub name: String,
    pub fields: Vec<IrField>,
    pub alignment: Option<u64>,
    pub is_vec: bool,
    pub flat_view: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrEnumVariant {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrEnum {
    pub name: String,
    pub underlying_type: PrimitiveType,
    pub variants: Vec<IrEnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrTypeDef {
    pub name: String,
    pub target: IrType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrItem {
    TypeDef(IrTypeDef),
    Struct(IrStruct),
    Enum(IrEnum),
    Function(IrFunction),
}

pub enum Symbol<'a> {
    Struct(&'a AstItem),
    Enum(&'a AstItem),
    TypeDef(&'a AstItem),
    Function(&'a AstItem),
}

pub type TypeDefsMap = HashMap<String, IrTypeDef>;
pub type StructsMap = HashMap<String, IrStruct>;
pub type EnumsMap = HashMap<String, IrEnum>;
pub type DirectFunctionsMap = HashMap<String, IrFunction>;
pub type DeferredFunctionsMap = HashMap<String, (u32, IrFunction)>;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct InterRepr {
    pub typedefs: TypeDefsMap,
    pub structs: StructsMap,
    pub enums: EnumsMap,
    pub direct_functions: DirectFunctionsMap,
    pub deferred_functions: DeferredFunctionsMap,
}

impl InterRepr {
    pub fn size_of(&self, ir_type_kind: &IrTypeKind) -> u64 {
        match ir_type_kind {
            IrTypeKind::Primitive(p) => p.byte_size(),
            IrTypeKind::Named(name) => {
                if let Some(ty) = self.typedefs.get(name) {
                    return self.size_of(&ty.target.kind);
                }
                if let Some(ty) = self.structs.get(name) {
                    let mut size: u64 = 0;
                    ty.fields.iter().for_each(|field| {
                        let field_size = self.size_of(&field.ty.kind);
                        let alignment = self.type_alignment(&field.ty.kind);
                        size = alignment.map_or(size, |alignment| size.next_multiple_of(alignment));
                        size += field_size;
                    });
                    return size;
                }
                if let Some(ty) = self.enums.get(name) {
                    return ty.underlying_type.byte_size();
                }
                panic!("Unknown type: {}", name);
            }
            IrTypeKind::FixedArray { element_type, size } => {
                size * self.size_of(&element_type.kind)
            }
            IrTypeKind::Pointer { .. }
            | IrTypeKind::Reference { .. }
            | IrTypeKind::FunctionPointer { .. } => 8,
        }
    }

    fn extract_struct_field(&self, ir_type_kind: &IrTypeKind) -> Option<&IrStruct> {
        match ir_type_kind {
            IrTypeKind::Named(name) => {
                if let Some(s) = self.structs.get(name) {
                    Some(s)
                } else if let Some(t) = self.typedefs.get(name) {
                    self.extract_struct_field(&t.target.kind)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn type_alignment(&self, ir_type_kind: &IrTypeKind) -> Option<u64> {
        self.extract_struct_field(ir_type_kind)?.alignment
    }
}

pub struct Lowerer<'a> {
    pub symbols: HashMap<String, Symbol<'a>>,
}

impl<'a> Default for Lowerer<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Lowerer<'a> {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    pub fn lower(&mut self, ast: &'a AbstractSyntaxTree) -> Result<InterRepr, IrError> {
        for item in &ast.items {
            match item {
                AstItem::TypeDef { name, .. } => {
                    self.symbols.insert(name.clone(), Symbol::TypeDef(item));
                }
                AstItem::Struct { name, .. } => {
                    self.symbols.insert(name.clone(), Symbol::Struct(item));
                }
                AstItem::Enum { name, .. } => {
                    self.symbols.insert(name.clone(), Symbol::Enum(item));
                }
                AstItem::Function { name, .. } => {
                    self.symbols.insert(name.clone(), Symbol::Function(item));
                }
            }
        }

        let mut ir = InterRepr::default();
        let mut deferred_count = 0;

        let mut structs_dag = DiGraphMap::<&str, ()>::new();

        for item in &ast.items {
            match item {
                AstItem::TypeDef { name, target } => {
                    ir.typedefs.insert(
                        name.clone(),
                        IrTypeDef {
                            name: name.clone(),
                            target: self.lower_type(target)?,
                        },
                    );
                }
                AstItem::Enum {
                    name,
                    underlying_type,
                    variants,
                } => {
                    let underlying_type =
                        underlying_type
                            .as_ref()
                            .map_or(Ok(PrimitiveType::I32), |ty| {
                                self.lower_type(ty).map(|ir_ty| {
                                    if let IrTypeKind::Primitive(primitive) = ir_ty.kind
                                        && matches!(
                                            primitive,
                                            PrimitiveType::I64
                                                | PrimitiveType::U64
                                                | PrimitiveType::I32
                                                | PrimitiveType::U32
                                                | PrimitiveType::I16
                                                | PrimitiveType::U16
                                                | PrimitiveType::I8
                                                | PrimitiveType::U8
                                        )
                                    {
                                        Ok(primitive)
                                    } else {
                                        Err(IrError::UnsupportedEnumUnderlyingType(name.clone()))
                                    }
                                })?
                            })?;

                    let mut next_value = 0i64;

                    let variants = variants
                        .iter()
                        .map(|variant| {
                            let value = variant.value.unwrap_or(next_value);
                            next_value = value + 1;
                            IrEnumVariant {
                                name: variant.name.clone(),
                                value,
                            }
                        })
                        .collect();

                    ir.enums.insert(
                        name.clone(),
                        IrEnum {
                            name: name.clone(),
                            underlying_type,
                            variants,
                        },
                    );
                }
                AstItem::Function {
                    name,
                    params,
                    return_type,
                    attributes,
                } => {
                    let ir_params = params
                        .iter()
                        .map(|p| {
                            Ok(IrParameter {
                                name: p.name.to_string(),
                                ty: self.lower_type(&p.ty)?,
                                is_out: p.attributes.is_out(),
                            })
                        })
                        .collect::<Result<Vec<IrParameter>, _>>()?;

                    let ir_return_type = self.lower_type(return_type)?;

                    if attributes.is_deferred() {
                        if ir_return_type.kind != IrTypeKind::Primitive(PrimitiveType::Void) {
                            return Err(IrError::UnsupportedEnumUnderlyingType(name.clone()));
                        }
                        ir.deferred_functions.insert(
                            name.clone(),
                            (
                                deferred_count,
                                IrFunction {
                                    name: name.to_string(),
                                    params: ir_params,
                                    return_type: ir_return_type,
                                    is_deferred: false,
                                },
                            ),
                        );
                        deferred_count += 1;
                    } else {
                        ir.direct_functions.insert(
                            name.clone(),
                            IrFunction {
                                name: name.to_string(),
                                params: ir_params,
                                return_type: ir_return_type,
                                is_deferred: true,
                            },
                        );
                    }
                }
                AstItem::Struct {
                    name,
                    fields,
                    alignment,
                    attributes,
                } => {
                    let mut ir_fields = Vec::new();

                    let is_vec = ir_fields.last().is_some_and(|f: &IrField| {
                        matches!(f.ty.kind, IrTypeKind::FixedArray { size: 0, .. })
                    });

                    for f in fields {
                        ir_fields.push(IrField {
                            name: f.name.clone(),
                            ty: self.lower_type(&f.ty)?,
                        });
                    }

                    ir.structs.insert(
                        name.clone(),
                        IrStruct {
                            name: name.clone(),
                            fields: ir_fields,
                            alignment: *alignment,
                            is_vec,
                            flat_view: attributes.has_flat_view(),
                        },
                    );
                }
            }
        }

        ir.structs.values().for_each(|ir_struct| {
            ir_struct.fields.iter().for_each(|f| {
                let f_struct = ir.extract_struct_field(&f.ty.kind);
                if f_struct.is_none() {
                    return;
                }
                structs_dag.add_edge(f_struct.unwrap().name.as_str(), ir_struct.name.as_str(), ());
            })
        });

        let updates: Vec<(String, Option<u64>)> = match toposort(&structs_dag, None) {
            Ok(order) => order
                .into_iter()
                .filter(|struct_name| ir.structs.contains_key(*struct_name))
                .map(|struct_name| {
                    let alignment = structs_dag
                        .neighbors_directed(struct_name, Direction::Incoming)
                        .filter_map(|f| ir.structs.get(f).and_then(|s| s.alignment))
                        .max();
                    (struct_name.to_owned(), alignment)
                })
                .collect(),
            Err(cycle) => panic!(
                "Error: Cyclic dependency detected involving struct: {}",
                cycle.node_id()
            ),
        };

        drop(structs_dag);

        for (struct_name, alignment) in updates {
            if let Some(ir_struct) = ir.structs.get_mut(&struct_name) {
                ir_struct.alignment = alignment;
            }
        }

        Ok(ir)
    }

    fn lower_type(&mut self, ty: &TypeKind) -> Result<IrType, IrError> {
        match ty {
            TypeKind::Named { name, is_const } => {
                let kind = if let Some(primitive) = self.match_primitive(name) {
                    IrTypeKind::Primitive(primitive)
                } else if self.symbols.contains_key(name) {
                    IrTypeKind::Named(name.clone())
                } else {
                    return Err(IrError::UnknownType(name.clone()));
                };
                Ok(IrType {
                    kind,
                    is_base_const: *is_const,
                })
            }
            TypeKind::Pointer { to, is_const } => Ok(IrType {
                kind: IrTypeKind::Pointer {
                    to: Box::new(self.lower_type(to)?),
                    is_ptr_const: *is_const,
                },
                is_base_const: false,
            }),
            TypeKind::FixedArray { element_type, size } => Ok(IrType {
                kind: IrTypeKind::FixedArray {
                    element_type: Box::new(self.lower_type(element_type)?),
                    size: *size,
                },
                is_base_const: false,
            }),
            TypeKind::FunctionPointer {
                return_type,
                param_types,
            } => Ok(IrType {
                kind: IrTypeKind::FunctionPointer {
                    return_type: Box::new(self.lower_type(return_type)?),
                    arguments: param_types
                        .iter()
                        .map(|t| self.lower_type(t))
                        .collect::<Result<Vec<IrType>, _>>()?,
                },
                is_base_const: false,
            }),
            TypeKind::Reference(ty) => Ok(IrType {
                kind: IrTypeKind::Reference {
                    to: Box::new(self.lower_type(ty)?),
                },
                is_base_const: false,
            }),
        }
    }

    #[inline]
    pub fn match_primitive(&self, name: &str) -> Option<PrimitiveType> {
        PrimitiveType::from(name).or_else(|| {
            let normalized: String = name
                .to_lowercase()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            PrimitiveType::from(&normalized)
        })
    }
}
