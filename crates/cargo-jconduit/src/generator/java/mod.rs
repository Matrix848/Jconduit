mod proxy;
mod view;

use crate::ProxySettings;
use crate::generator::java::proxy::ProxyEmitter;
use crate::generator::java::view::ViewEmitter;
use crate::ir::{InterRepr, IrTypeKind, PrimitiveType};
use askama::Template;
use std::fs;
use std::path::Path;

fn primitive_to_java_layout(p: &PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Bool => "ValueLayout.JAVA_BOOLEAN",
        PrimitiveType::U8 | PrimitiveType::I8 => "ValueLayout.JAVA_BYTE",
        PrimitiveType::U16 | PrimitiveType::I16 => "ValueLayout.JAVA_SHORT",
        PrimitiveType::U32 | PrimitiveType::I32 | PrimitiveType::Wchar => "ValueLayout.JAVA_INT",
        PrimitiveType::U64
        | PrimitiveType::I64
        | PrimitiveType::SizeT
        | PrimitiveType::SsizeT
        | PrimitiveType::Uptr
        | PrimitiveType::Iptr
        | PrimitiveType::PtrDiffT => "ValueLayout.JAVA_LONG",
        PrimitiveType::Float => "ValueLayout.JAVA_FLOAT",
        PrimitiveType::Double => "ValueLayout.JAVA_DOUBLE",
        PrimitiveType::Void => panic!("Void type does not have a layout"),
    }
}

fn primitive_to_java_type(p: &PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Bool => "boolean",
        PrimitiveType::U8 | PrimitiveType::I8 => "byte",
        PrimitiveType::U16 | PrimitiveType::I16 => "short",
        PrimitiveType::U32 | PrimitiveType::I32 | PrimitiveType::Wchar => "int",
        PrimitiveType::U64
        | PrimitiveType::I64
        | PrimitiveType::SizeT
        | PrimitiveType::SsizeT
        | PrimitiveType::Uptr
        | PrimitiveType::Iptr
        | PrimitiveType::PtrDiffT => "long",
        PrimitiveType::Float => "float",
        PrimitiveType::Double => "double",
        PrimitiveType::Void => "void",
    }
}

#[derive(Debug, Clone)]
pub enum WriteNode {
    Set {
        java_param: String,
        offset_expr: String,
        layout: String,
    },
}

#[derive(Debug, Clone)]
struct FlatParam {
    java_name: String,
    java_type: String,
}

struct TemplateFn {
    signature: String,
    body: String,
}

#[inline]
fn format_signature(return_type: &str, java_name: &str, flat: &[FlatParam]) -> String {
    let params = flat
        .iter()
        .map(|p| format!("{} {}", p.java_type, p.java_name))
        .collect::<Vec<_>>()
        .join(", ");

    format!("public {return_type} {java_name}({params})")
}

pub type JavaRepr = InterRepr;

impl JavaRepr {
    fn map_ir_type_kind_to_java(&self, ir_type_kind: &IrTypeKind) -> String {
        match ir_type_kind {
            IrTypeKind::Primitive(p) => primitive_to_java_type(p).to_string(),
            IrTypeKind::Named(name) => {
                if let Some(e) = self.enums.get(name) {
                    primitive_to_java_type(&e.underlying_type).to_string()
                } else {
                    "MemorySegment".to_string()
                }
            }
            _ => "MemorySegment".to_string(),
        }
    }

    fn map_ir_type_kind_to_java_layout(&self, ir_type_kind: &IrTypeKind) -> String {
        match ir_type_kind {
            IrTypeKind::Primitive(p) => primitive_to_java_layout(p).to_string(),
            IrTypeKind::Named(name) => {
                if let Some(e) = self.enums.get(name) {
                    primitive_to_java_layout(&e.underlying_type).to_string()
                } else {
                    format!("{}.$LAYOUT", name)
                }
            }
            IrTypeKind::FixedArray { element_type, size } => {
                let element_layout = self.map_ir_type_kind_to_java_layout(&element_type.kind);
                format!("MemoryLayout.sequenceLayout({}, {})", size, element_layout)
            }
            IrTypeKind::Pointer {
                to,
                is_ptr_const: _,
            } => match to.kind.clone() {
                IrTypeKind::Primitive(PrimitiveType::Void) => "ValueLayout.ADDRESS".to_string(),

                IrTypeKind::Named(name) => {
                    if self.structs.contains_key(&name) {
                        format!("ValueLayout.ADDRESS.withTargetLayout({}.$LAYOUT)", name)
                    } else if let Some(e) = self.enums.get(&name) {
                        let prim = primitive_to_java_layout(&e.underlying_type);
                        format!("ValueLayout.ADDRESS.withTargetLayout({})", prim)
                    } else {
                        "ValueLayout.ADDRESS".to_string()
                    }
                }
                _ => "ValueLayout.ADDRESS".to_string(),
            },
            _ => "ValueLayout.ADDRESS".to_string(),
        }
    }
}

pub fn java_gen(
    java_dir: &Path,
    jcd_package: &str,
    jxt_package: &str,
    jxt_class_name: &str,
    proxy_class_name: &str,
    proxy_settings: &ProxySettings,
    ir: &InterRepr,
) -> anyhow::Result<()> {
    let out_dir = java_dir.join(jcd_package.replace(".", "/"));
    fs::create_dir_all(&out_dir)?;

    let proxy = ProxyEmitter::new(
        jcd_package,
        jxt_package,
        jxt_class_name,
        proxy_class_name,
        proxy_settings,
        ir,
    )
    .emit();

    fs::write(
        out_dir.join(format!("{}.java", proxy_class_name)),
        proxy.render()?,
    )?;

    let view = ViewEmitter::new(jcd_package, jxt_class_name, proxy_class_name, ir).emit("s");

    Ok(())
}
