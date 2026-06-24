use crate::generator::java::{JavaRepr, TemplateFn};
use crate::ir::{InterRepr, IrTypedVar};
use crate::utils::formatting::{to_camel_case, to_pascal_case};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ViewEmitterErr {}

pub struct ViewEmitter<'a> {
    package: String,
    jextract_class_name: String,
    proxy_class_name: String,
    jr: &'a JavaRepr,
}

impl<'a> ViewEmitter<'a> {
    pub fn new(
        package: impl Into<String>,
        jextract_class_name: impl Into<String>,
        proxy_class_name: impl Into<String>,
        jr: &'a InterRepr,
    ) -> Self {
        Self {
            package: package.into(),
            jextract_class_name: jextract_class_name.into(),
            proxy_class_name: proxy_class_name.into(),
            jr,
        }
    }

    pub fn emit(&self, of: &str) -> Result<String, ViewEmitterErr> {
        let ir_struct = self.jr.structs.get("struct");

        Ok("".to_string())
    }

    fn emit_get_offset(&self, ir_typed_var: IrTypedVar, offset: u64) -> TemplateFn {
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

    fn emit_get_value(&self, parent: &str, ir_typed_var: IrTypedVar, offset: u64) -> TemplateFn {
        let return_kind = &ir_typed_var.ty.kind;
        let offset_var_name = format!("{}Offset", to_camel_case(parent));

        let signature = format!(
            "public {} get{}(MemorySegment memorySegment, long {})",
            self.jr.map_ir_type_kind_to_java(return_kind),
            to_pascal_case(&ir_typed_var.name),
            offset_var_name
        );
        TemplateFn {
            signature,
            body: format!(
                "return memorySegment.get({}, {} + {})",
                self.jr.map_ir_type_kind_to_java_layout(return_kind),
                offset_var_name,
                offset
            ),
        }
    }
}
