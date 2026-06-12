use crate::parser::{AbstractSyntaxTree, AstItem, HandleType, TypeKind};
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
    #[error("Struct: {0} is not a vector.")]
    CountFieldOnNonVecStruct(String),
    #[error("Invalid type for vector {0} count. Only size_t and ptrdiff_t are supported.")]
    InvalidCountType(String),
    #[error("Struct: {0} does not have a count field but is marked as a vector.")]
    MissingCountFieldOnVecStruct(String),
    #[error(
        "Struct: {0} has a duplicated count field. Only one count field is allowed per struct."
    )]
    DuplicatedCountField(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Void,
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    Float,
    Double,
    SizeT,
    SsizeT,
    PtrDiffT,
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

            // Platform-Dependent Architecture Integers (Fallback mappings for standard 64-bit targets)
            "long" | "longint" | "signedlong" | "signedlongint" => Some(PrimitiveType::I64),
            "unsignedlong" | "unsignedlongint" => Some(PrimitiveType::U64),

            // Floating Point
            "float" => Some(PrimitiveType::Float),
            "double" | "longdouble" => Some(PrimitiveType::Double),

            // Size & Pointer Arithmetics
            "size_t" => Some(PrimitiveType::SizeT),
            "ssize_t" => Some(PrimitiveType::SsizeT),
            "ptrdiff_t" => Some(PrimitiveType::PtrDiffT),
            "uintptr_t" => Some(PrimitiveType::Uptr),
            "intptr_t" => Some(PrimitiveType::Iptr),

            // OS Specific Wide Strings
            "wchar_t" => Some(PrimitiveType::Wchar),

            // Not a primitive (likely a User-Defined Class, Struct, or Enum)
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IrTypeKind {
    Primitive(PrimitiveType),
    UserDefined(String),
    Pointer {
        to: Box<IrType>,
        is_ptr_const: bool,
    },
    Reference {
        to: Box<IrType>,
    },
    FixedArray {
        element_type: Box<IrType>,
        size: usize,
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
    pub is_direct: bool,
    pub no_scratchpad: bool,
    pub out_handle: Option<HandleType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrStruct {
    pub name: String,
    pub fields: Vec<IrField>,
    pub alignment: Option<usize>,
    pub is_vec: bool,
    pub count_field_idx: Option<usize>,
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

#[derive(Debug, Clone, Default)]
pub struct UnifiedRepresentation {
    pub typedefs: Vec<IrTypeDef>,
    pub structs: Vec<IrStruct>,
    pub enums: Vec<IrEnum>,
    pub functions: Vec<IrFunction>,
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

    pub fn lower(&mut self, ast: &'a AbstractSyntaxTree) -> Result<UnifiedRepresentation, IrError> {
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

        let mut ir = UnifiedRepresentation::default();

        for item in &ast.items {
            match item {
                AstItem::TypeDef { name, target } => {
                    ir.typedefs.push(IrTypeDef {
                        name: name.clone(),
                        target: self.lower_type(target)?,
                    });
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

                    ir.enums.push(IrEnum {
                        name: name.clone(),
                        underlying_type,
                        variants,
                    })
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

                    if ir_return_type.kind == IrTypeKind::Primitive(PrimitiveType::Void) {
                        ir.functions.push(IrFunction {
                            name: name.to_string(),
                            params: ir_params,
                            return_type: ir_return_type,
                            is_direct: attributes.is_direct(),
                            no_scratchpad: attributes.is_no_scratchpad(),
                            out_handle: attributes.get_handle_type(),
                        });
                        continue;
                    }

                    ir.functions.push(IrFunction {
                        name: name.to_string(),
                        params: ir_params,
                        return_type: ir_return_type,
                        is_direct: true,
                        no_scratchpad: attributes.is_no_scratchpad(),
                        out_handle: attributes.get_handle_type(),
                    });
                }
                AstItem::Struct {
                    name,
                    fields,
                    alignment,
                    is_vec,
                } => {
                    let mut ir_fields = Vec::new();
                    let mut count_field_idx = None;

                    if *is_vec {
                        for (i, f) in fields.iter().enumerate() {
                            let field_ty = self.lower_type(&f.ty)?;

                            let is_count = f.attributes.is_count();
                            if is_count {
                                if let IrTypeKind::Primitive(primitive) = field_ty.kind
                                    && matches!(
                                        primitive,
                                        PrimitiveType::SizeT | PrimitiveType::PtrDiffT
                                    )
                                {
                                    if count_field_idx.is_some() {
                                        return Err(IrError::DuplicatedCountField(name.clone()));
                                    }
                                    count_field_idx = Some(i);
                                } else {
                                    return Err(IrError::InvalidCountType(name.clone()));
                                }
                            }

                            ir_fields.push(IrField {
                                name: f.name.clone(),
                                ty: field_ty,
                            });
                        }

                        if count_field_idx.is_none() {
                            return Err(IrError::MissingCountFieldOnVecStruct(name.clone()));
                        }
                    } else {
                        for f in fields {
                            if f.attributes.is_count() {
                                return Err(IrError::CountFieldOnNonVecStruct(name.clone()));
                            }

                            ir_fields.push(IrField {
                                name: f.name.clone(),
                                ty: self.lower_type(&f.ty)?,
                            });
                        }
                    }

                    ir.structs.push(IrStruct {
                        name: name.clone(),
                        fields: ir_fields,
                        alignment: *alignment,
                        is_vec: *is_vec,
                        count_field_idx,
                    });
                }
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
                    IrTypeKind::UserDefined(name.clone())
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
