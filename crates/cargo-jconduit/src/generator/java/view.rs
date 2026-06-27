use crate::generator::java::{JavaRepr, TemplateFn};
use crate::ir::{InterRepr, IrTypedVar};
use crate::utils::formatting::{to_camel_case, to_pascal_case};
use askama::Template;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ViewEmitterErr {
    #[error(
        "Cannot create view for type: {0}. Views only support struct(or typedefs which resolve into structs)."
    )]
    NonStructView(String),
}

#[derive(Template)]
#[template(path = "view.java", escape = "none")]
pub struct ViewTemplate {
    class_name: String,
    package: String,
    functions: Vec<TemplateFn>,
}

pub struct ViewEmitter<'a> {
    package: String,
    jxt_class_name: String,
    proxy_class_name: String,
    jr: &'a JavaRepr,
}

impl<'a> ViewEmitter<'a> {
    pub fn new(
        package: impl Into<String>,
        jxt_class_name: impl Into<String>,
        proxy_class_name: impl Into<String>,
        jr: &'a InterRepr,
    ) -> Self {
        Self {
            package: package.into(),
            jxt_class_name: jxt_class_name.into(),
            proxy_class_name: proxy_class_name.into(),
            jr,
        }
    }

    pub fn emit(&self, of: &str) -> Result<ViewTemplate, ViewEmitterErr> {
        let ir_struct = self
            .jr
            .resolve_inner_struct(of)
            .ok_or(ViewEmitterErr::NonStructView(of.into()))?;

        let mut functions = Vec::new();

        let mut offset = 0;

        for field in ir_struct.fields.iter() {
            let typed_var = field.clone().into();
            functions.push(self.emit_get_offset(&typed_var, offset));
            functions.push(self.emit_get_value(None, &typed_var, offset));
            offset += self.jr.size_of(&field.ty.kind);
        }

        Ok(ViewTemplate {
            class_name: format!("View{}", of),
            package: self.package.clone(),
            functions,
        })
    }

    fn emit_get_offset(&self, ir_typed_var: &IrTypedVar, offset: u64) -> TemplateFn {
        let field_name = to_pascal_case(&ir_typed_var.name);
        let index_name = format!("{}Index", field_name);

        let signature = format!(
            "public long get{}Offset(MemorySegment memorySegment, int {})",
            field_name, index_name
        );

        TemplateFn {
            signature,
            body: format!(
                "return (long) buffer.get(ValueLayout.JAVA_INT, ({} * {}L))",
                index_name,
                self.jr.size_of(&ir_typed_var.ty.kind) + offset
            ),
        }
    }

    fn emit_get_value(
        &self,
        parent: Option<&str>,
        ir_typed_var: &IrTypedVar,
        offset: u64,
    ) -> TemplateFn {
        let return_kind = &ir_typed_var.ty.kind;
        let java_type = self.jr.map_ir_type_kind_to_java(return_kind);
        let java_layout = self.jr.map_ir_type_kind_to_java_layout(return_kind);
        let method_name = to_pascal_case(&ir_typed_var.name);

        let (signature, body) = match parent {
            Some(p) => {
                let offset_var = format!("{}Offset", to_camel_case(p));
                let sig = format!(
                    "public {} get{}(MemorySegment memorySegment, long {})",
                    java_type, method_name, offset_var
                );
                let bdy = format!(
                    "return memorySegment.get({}, {} + {});",
                    java_layout, offset, offset_var
                );
                (sig, bdy)
            }
            None => {
                let sig = format!(
                    "public {} get{}(MemorySegment memorySegment)",
                    java_type, method_name
                );
                let bdy = format!("return memorySegment.get({}, {});", java_layout, offset);
                (sig, bdy)
            }
        };

        TemplateFn { signature, body }
    }
}
