use crate::generator::ForeignFunctions;
use crate::utils::formatting::{capitalize, to_camel_case, IndentWriter};
use crate::ProxySettings;
use anyhow::{bail, Context, Result};
use askama::Template;
use cbindgen::ParseConfig;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use syn::{Fields, File, FnArg, Item, PatType, ReturnType, Signature, Type, TypePath};

pub type StructMap = HashMap<String, RustStruct>;
pub type EnumMap = HashMap<String, RustEnum>;

#[derive(Debug, Clone, Copy)]
pub enum Primitive {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
}

impl Primitive {
    pub fn java_type(self) -> &'static str {
        match self {
            Self::U8 | Self::I8 => "byte",
            Self::U16 | Self::I16 => "short",
            Self::U32 | Self::I32 => "int",
            Self::U64 | Self::I64 => "long",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Bool => "boolean",
        }
    }

    fn java_value_layout(self) -> &'static str {
        match self {
            Self::U8 | Self::I8 => "JAVA_BYTE",
            Self::U16 | Self::I16 => "JAVA_SHORT",
            Self::U32 | Self::I32 => "JAVA_INT",
            Self::U64 | Self::I64 => "JAVA_LONG",
            Self::F32 => "JAVA_FLOAT",
            Self::F64 => "JAVA_DOUBLE",
            Self::Bool => "JAVA_BOOLEAN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pointer {
    pub inner: Box<RustType>,
    pub is_const: bool,
}

impl Pointer {
    pub const fn java_layout() -> &'static str {
        "ADDRESS"
    }
}

#[derive(Debug, Clone)]
pub enum RustType {
    Primitive(Primitive),
    Struct(String),
    Ptr(Pointer),
}

impl RustType {
    pub fn java_layout(&self) -> String {
        match self {
            Self::Primitive(p) => p.java_value_layout().to_string(),
            Self::Struct(s) => format!("{}.LAYOUT", s),
            Self::Ptr(_) => Pointer::java_layout().to_string(),
        }
    }

    pub fn java_type(&self) -> &str {
        match self {
            Self::Primitive(p) => p.java_type(),
            Self::Struct(s) => s.as_str(),
            Self::Ptr(_) => "MemorySegment",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RustField {
    pub name: String,
    pub ty: RustType,
}

#[derive(Debug, Clone)]
pub struct RustStruct {
    pub name: String,
    pub fields: Vec<RustField>,
}

impl RustStruct {
    pub fn parse_all(file: &File) -> StructMap {
        file.items
            .iter()
            .filter_map(|item| {
                let Item::Struct(s) = item else {
                    return None;
                };
                if !has_repr_c(&s.attrs) {
                    return None;
                }

                let Fields::Named(named) = &s.fields else {
                    return None;
                };
                let fields = named
                    .named
                    .iter()
                    .filter_map(|f| {
                        Some(RustField {
                            name: f.ident.as_ref()?.to_string(),
                            ty: RustType::try_from(&f.ty).ok()?,
                        })
                    })
                    .collect();

                Some((
                    s.ident.to_string(),
                    Self {
                        name: s.ident.to_string(),
                        fields,
                    },
                ))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RustEnum {
    pub repr: Primitive,
}

impl RustEnum {
    pub fn parse_all(file: &File) -> EnumMap {
        file.items
            .iter()
            .filter_map(|item| {
                let Item::Enum(e) = item else {
                    return None;
                };
                let repr = enum_repr(&e.attrs)?;
                Some((e.ident.to_string(), Self { repr }))
            })
            .collect()
    }
}

fn enum_repr(attrs: &[syn::Attribute]) -> Option<Primitive> {
    for attr in attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut prim: Option<Primitive> = None;
        let _ = attr.parse_nested_meta(|meta| {
            if let Some(ident) = meta.path.get_ident() {
                prim = match ident.to_string().as_str() {
                    "u8" => Some(Primitive::U8),
                    "u16" => Some(Primitive::U16),
                    "u32" => Some(Primitive::U32),
                    "u64" => Some(Primitive::U64),
                    "i8" => Some(Primitive::I8),
                    "i16" => Some(Primitive::I16),
                    "i32" => Some(Primitive::I32),
                    "i64" => Some(Primitive::I64),
                    _ => None,
                };
            }
            Ok(())
        });
        if prim.is_some() {
            return prim;
        }
    }
    None
}

fn has_repr_c(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("repr") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("C") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

impl TryFrom<&Type> for RustType {
    type Error = &'static str;

    fn try_from(ty: &Type) -> Result<Self, Self::Error> {
        match ty {
            Type::Ptr(ptr) => {
                let inner = Box::new(Self::try_from(&*ptr.elem)?);
                Ok(Self::Ptr(Pointer {
                    inner,
                    is_const: ptr.const_token.is_some(),
                }))
            }
            Type::Path(TypePath { path, .. }) => {
                let ident = path
                    .segments
                    .last()
                    .ok_or("Empty segment path")?
                    .ident
                    .to_string();
                Ok(match ident.as_str() {
                    "u8" => Self::Primitive(Primitive::U8),
                    "u16" => Self::Primitive(Primitive::U16),
                    "u32" => Self::Primitive(Primitive::U32),
                    "u64" => Self::Primitive(Primitive::U64),
                    "i8" => Self::Primitive(Primitive::I8),
                    "i16" => Self::Primitive(Primitive::I16),
                    "i32" => Self::Primitive(Primitive::I32),
                    "i64" => Self::Primitive(Primitive::I64),
                    "f32" => Self::Primitive(Primitive::F32),
                    "f64" => Self::Primitive(Primitive::F64),
                    "bool" => Self::Primitive(Primitive::Bool),
                    other => Self::Struct(other.to_string()),
                })
            }
            _ => Err("Unsupported type variant"),
        }
    }
}

pub fn params_from_sig(sig: &Signature) -> Vec<RustField> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            let FnArg::Typed(PatType { pat, ty, .. }) = arg else {
                return None;
            };
            let syn::Pat::Ident(pi) = pat.as_ref() else {
                return None;
            };
            Some(RustField {
                name: pi.ident.to_string(),
                ty: RustType::try_from(&**ty).ok()?,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct FlatParam {
    pub java_name: String,
    pub java_type: String,
}

pub fn flatten_params(
    params: &[RustField],
    structs: &StructMap,
    enums: &EnumMap,
) -> Vec<FlatParam> {
    let mut out = Vec::new();
    for param in params {
        flatten_field(param, "", structs, enums, &mut out);
    }
    out
}

fn flatten_field(
    field: &RustField,
    prefix: &str,
    structs: &StructMap,
    enums: &EnumMap,
    out: &mut Vec<FlatParam>,
) {
    let java_name = if prefix.is_empty() {
        to_camel_case(&field.name)
    } else {
        format!("{}{}", to_camel_case(prefix), capitalize(&field.name))
    };

    match &field.ty {
        RustType::Primitive(p) => out.push(FlatParam {
            java_name,
            java_type: p.java_type().to_string(),
        }),
        RustType::Ptr(_) => out.push(FlatParam {
            java_name,
            java_type: "MemorySegment".to_string(),
        }),
        RustType::Struct(s) => {
            if let Some(e) = enums.get(s) {
                out.push(FlatParam {
                    java_name,
                    java_type: e.repr.java_type().to_string(),
                });
                return;
            }
            let s = structs.get(s).expect("unknown struct mapping encountered");
            let next_prefix = if prefix.is_empty() {
                field.name.clone()
            } else {
                format!("{}_{}", prefix, field.name)
            };
            for child in &s.fields {
                flatten_field(child, &next_prefix, structs, enums, out);
            }
        }
    }
}

pub struct GeneratedFn {
    pub signature: String,
    pub body: String,
}

impl GeneratedFn {
    fn format_signature(return_type: &str, java_name: &str, flat: &[FlatParam]) -> String {
        let params = flat
            .iter()
            .map(|p| format!("{} {}", p.java_type, p.java_name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("public {} {}({})", return_type, java_name, params)
    }
}

pub fn generate_deferred_fn(
    cmd_id: u32,
    sig: &Signature,
    structs: &StructMap,
    enums: &EnumMap,
) -> Result<GeneratedFn> {
    let fn_name = sig.ident.to_string();
    let java_name = to_camel_case(&fn_name);
    let cmd_class = format!("Command{}", capitalize(&java_name));

    let params = params_from_sig(sig);
    let flat = flatten_params(&params, structs, enums);
    let signature = GeneratedFn::format_signature("void", &java_name, &flat);

    let mut w = IndentWriter::new(1);
    w.line(&format!(
        "long payloadSize = {cmd_class}.layout().byteSize();"
    ));
    w.indent();
    w.line(&format!(
        "long payloadCursor = this.reserveCommandSpace(threadState, payloadSize, {cmd_id});"
    ));
    w.line("");

    let nodes = build_write_nodes(&params, structs, enums, &cmd_class, "", "");
    emit_write_nodes(&nodes, &mut w)?;

    Ok(GeneratedFn {
        signature,
        body: w.finish(),
    })
}

pub fn generate_direct_fn(
    sig: &Signature,
    jextract_class: &str,
    structs: &StructMap,
    enums: &EnumMap,
) -> Result<GeneratedFn> {
    let fn_name = sig.ident.to_string();
    let java_name = to_camel_case(&fn_name);

    let params = params_from_sig(sig);
    let flat = flatten_params(&params, structs, enums);

    let return_type = match &sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let mut rt = RustType::try_from(&**ty)
                .map_err(|_| anyhow::anyhow!("unsupported return type in {fn_name}"))?;
            if let RustType::Struct(name) = &rt {
                if let Some(e) = enums.get(name) {
                    rt = RustType::Primitive(e.repr);
                }
            }
            Some(rt)
        }
    };

    let return_type_str = return_type
        .as_ref()
        .map(|t| t.java_type())
        .unwrap_or("void");
    let signature = GeneratedFn::format_signature(return_type_str, &java_name, &flat);

    let args = flat
        .iter()
        .map(|p| p.java_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let call = format!("{jextract_class}.jconduit_{fn_name}({args})");

    let mut w = IndentWriter::new(1);
    match return_type {
        None => w.line(&format!("{call};")),
        Some(RustType::Ptr(ptr)) => {
            w.line(&format!("MemorySegment ptr = {call};"));
            w.indent();
            w.line("");
            w.line("if (ptr.equals(MemorySegment.NULL)) {");
            w.indent();
            w.line(&format!(
                "throw new NullPointerException(\"{fn_name} returned a NULL pointer\");"
            ));
            w.dedent();
            w.line("}");
            w.line("");

            let layout = format!("{}.byteSize()", ptr.inner.java_layout());
            if ptr.is_const {
                w.line(&format!(
                    "return ptr.reinterpret({layout}).asReadOnlySegment();"
                ));
            } else {
                w.line(&format!("return ptr.reinterpret({layout});"));
            }
        }
        Some(_) => w.line(&format!("return {call};")),
    }

    Ok(GeneratedFn {
        signature,
        body: w.finish(),
    })
}

pub fn gen_scratchpad_overloads(
    sig: &Signature,
    jextract_class: &str,
    structs: &StructMap,
    enums: &EnumMap,
) -> Result<(GeneratedFn, String)> {
    let fn_name = sig.ident.to_string();
    let java_name = to_camel_case(&fn_name);

    let params = params_from_sig(sig);
    let Some((out, input_params)) = params.split_last() else {
        bail!("Missing arguments inside scratchpad definition context: {fn_name}");
    };

    let RustType::Ptr(out_ptr) = &out.ty else {
        bail!("scratchpad overload output must be a pointer: {fn_name}");
    };
    if matches!(&sig.output, ReturnType::Type(_, _)) {
        bail!("scratchpad overloads cannot return a value: {fn_name}");
    }

    let flat = flatten_params(input_params, structs, enums);
    let args = flat
        .iter()
        .map(|p| p.java_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut w = IndentWriter::new(1);

    if let RustType::Struct(name) = &*out_ptr.inner
        && let Some(e) = enums.get(name)
    {
        let layout = format!("ValueLayout.{}", e.repr.java_value_layout());
        let signature = GeneratedFn::format_signature(e.repr.java_type(), &java_name, &flat);
        w.line(&format!(
            "MemorySegment scratchpad = this.reserveScratchpad({layout});"
        ));
        w.indent();
        w.line(&format!(
            "{jextract_class}.jconduit_{fn_name}({args}, scratchpad);"
        ));
        w.line(&format!("return scratchpad.get({layout}, 0);"));

        return Ok((
            GeneratedFn {
                signature,
                body: w.finish(),
            },
            format!("{}.byteSize()", layout),
        ));
    }

    let layout = out_ptr.inner.java_layout();
    let signature = GeneratedFn::format_signature("MemorySegment", &java_name, &flat);
    w.line(&format!(
        "MemorySegment scratchpad = this.reserveScratchpad({layout});"
    ));
    w.indent();
    w.line(&format!("{jextract_class}.{fn_name}({args}, scratchpad);"));
    w.line(&format!(
        "return scratchpad.reinterpret({layout}.byteSize()).asReadOnlySegment();"
    ));

    Ok((
        GeneratedFn {
            signature,
            body: w.finish(),
        },
        format!("{}.byteSize()", layout),
    ))
}

enum WriteNode {
    Set {
        layout_name: String,
        offset_expr: String,
        java_param: String,
    },
}

fn build_write_nodes(
    fields: &[RustField],
    structs: &StructMap,
    enums: &EnumMap,
    parent_class: &str,
    offset_prefix: &str,
    name_prefix: &str,
) -> Vec<WriteNode> {
    fields
        .iter()
        .flat_map(|field| {
            let java_base = if name_prefix.is_empty() {
                to_camel_case(&field.name)
            } else {
                format!("{}{}", to_camel_case(name_prefix), capitalize(&field.name))
            };

            match &field.ty {
                RustType::Primitive(primitive) => vec![WriteNode::Set {
                    layout_name: primitive.java_value_layout().to_string(),
                    offset_expr: format!("{offset_prefix}{parent_class}.{}$offset()", field.name),
                    java_param: java_base,
                }],
                RustType::Ptr(_) => vec![WriteNode::Set {
                    layout_name: "ADDRESS".to_string(),
                    offset_expr: format!("{offset_prefix}{parent_class}.{}$offset()", field.name),
                    java_param: java_base,
                }],
                RustType::Struct(struct_name) => {
                    if let Some(e) = enums.get(struct_name) {
                        return vec![WriteNode::Set {
                            layout_name: e.repr.java_value_layout().to_string(),
                            offset_expr: format!(
                                "{offset_prefix}{parent_class}.{}$offset()",
                                field.name
                            ),
                            java_param: java_base,
                        }];
                    }
                    let s = structs
                        .get(struct_name)
                        .expect("unknown structural member mapping target");
                    let new_offset_prefix =
                        format!("{offset_prefix}{parent_class}.{}$offset() + ", field.name);
                    let new_name_prefix = if name_prefix.is_empty() {
                        field.name.clone()
                    } else {
                        format!("{}_{}", name_prefix, field.name)
                    };
                    build_write_nodes(
                        &s.fields,
                        structs,
                        enums,
                        struct_name,
                        &new_offset_prefix,
                        &new_name_prefix,
                    )
                }
            }
        })
        .collect()
}

fn emit_write_nodes(nodes: &[WriteNode], w: &mut IndentWriter) -> Result<()> {
    for WriteNode::Set {
        layout_name,
        offset_expr,
        java_param,
    } in nodes
    {
        w.line(&format!(
            "threadState.buffer.set(ValueLayout.{layout_name}, payloadCursor + {offset_expr}, {java_param});"
        ));
    }
    Ok(())
}

pub(super) fn gen_proxy_bindings(
    typedef_header: &Path,
    source_files: &[&PathBuf],
    exclude: Vec<String>,
    output_file: &Path,
) -> Result<()> {
    let config = cbindgen::Config {
        parse: ParseConfig::default(),
        pragma_once: true,
        after_includes: Some("#include \"typedef.h\"".to_string()),
        export: cbindgen::ExportConfig {
            exclude,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut builder = cbindgen::Builder::new()
        .with_config(config)
        .with_language(cbindgen::Language::C);
    for source in source_files {
        builder = builder.with_src(*source);
    }

    builder.generate()?.write_to_file(output_file);
    fs::copy(
        typedef_header,
        output_file.parent().unwrap().join("typedef.h"),
    )?;

    Ok(())
}

pub(super) fn jextract_bindings(
    package: &str,
    jextract_class_name: &str,
    header_path: &Path,
    output_dir: &Path,
) -> Result<()> {
    #[cfg(target_os = "windows")]
    const JEXTRACT_CMD: &str = "jextract.bat";

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    const JEXTRACT_CMD: &str = "jextract";

    Command::new(JEXTRACT_CMD)
        .arg("--version")
        .output()
        .context(
            "❌ jextract execution verification failed. Download: https://jdk.java.net/jextract/",
        )?;

    Command::new(JEXTRACT_CMD)
        .arg("--output")
        .arg(output_dir)
        .arg("--target-package")
        .arg(package)
        .arg("--header-class-name")
        .arg(jextract_class_name)
        .arg(header_path)
        .output()?;

    Ok(())
}

#[derive(Template)]
#[template(path = "proxy.java", escape = "none")]
pub(super) struct ProxyTemplate {
    pub package: String,
    pub jextract_class_name: String,
    pub proxy_class_name: String,
    pub deferred_fns: Vec<GeneratedFn>,
    pub direct_fns: Vec<GeneratedFn>,
    pub direct_scratchpad_fns: Vec<GeneratedFn>,
    pub proxy_settings: ProxySettings,
    pub has_scratchpad_overloads: bool,
    pub scratchpad_layouts_byte_sizes: String,
}

pub fn generate_proxy_template(
    package: &str,
    jextract_class_name: &str,
    proxy_class_name: &str,
    proxy_settings: &ProxySettings,
    foreign_functions: ForeignFunctions,
    structs: &StructMap,
    enums: &EnumMap,
) -> Result<ProxyTemplate> {
    let deferred_fns = foreign_functions
        .deferred
        .iter()
        .map(|(cmd_id, sig)| generate_deferred_fn(*cmd_id, sig, structs, enums))
        .collect::<Result<Vec<_>>>()?;

    let direct_fns = foreign_functions
        .direct
        .iter()
        .map(|sig| generate_direct_fn(sig, jextract_class_name, structs, enums))
        .collect::<Result<Vec<_>>>()?;

    let (direct_scratchpad_fns, out_byte_size) = foreign_functions
        .direct_scratchpad
        .iter()
        .map(|sig| gen_scratchpad_overloads(sig, jextract_class_name, structs, enums))
        .collect::<Result<(Vec<_>, Vec<_>)>>()?;

    let scratchpad_layouts_byte_sizes = out_byte_size.join(", ");
    let has_scratchpad_overloads = !direct_scratchpad_fns.is_empty();

    Ok(ProxyTemplate {
        package: package.to_string(),
        jextract_class_name: jextract_class_name.to_string(),
        proxy_class_name: proxy_class_name.to_string(),
        deferred_fns,
        direct_fns,
        direct_scratchpad_fns,
        proxy_settings: *proxy_settings,
        has_scratchpad_overloads,
        scratchpad_layouts_byte_sizes,
    })
}
