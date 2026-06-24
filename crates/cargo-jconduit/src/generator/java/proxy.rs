use crate::ProxySettings;
use crate::generator::java::{
    FlatParam, JavaRepr, TemplateFn, format_signature, primitive_to_java_layout,
    primitive_to_java_type,
};
use crate::ir::{
    InterRepr, IrFunction, IrParameter, IrStruct, IrType, IrTypeKind, IrTypedVar, PrimitiveType,
};
use crate::utils::formatting::{Writer, capitalize, to_camel_case, to_pascal_case};
use askama::Template;

#[derive(Template)]
#[template(path = "proxy.java", escape = "none")]
pub struct ProxyTemplate {
    package: String,
    jxt_package: String,
    jextract_class_name: String,
    proxy_class_name: String,
    deferred_fns: Vec<TemplateFn>,
    direct_fns: Vec<TemplateFn>,
    proxy_settings: ProxySettings,
    has_scratchpad_overloads: bool,
    scratchpad_layouts_byte_sizes: String,
}

pub struct ProxyEmitter<'a> {
    package: String,
    jxt_package: String,
    jextract_class_name: String,
    proxy_class_name: String,
    proxy_settings: ProxySettings,
    jr: &'a InterRepr,
    views: Vec<IrStruct>,
}

impl<'a> ProxyEmitter<'a> {
    pub fn new(
        package: impl Into<String>,
        jxt_package: impl Into<String>,
        jextract_class_name: impl Into<String>,
        proxy_class_name: impl Into<String>,
        proxy_settings: &ProxySettings,
        jr: &'a JavaRepr,
    ) -> Self {
        Self {
            package: package.into(),
            jxt_package: jxt_package.into(),
            jextract_class_name: jextract_class_name.into(),
            proxy_class_name: proxy_class_name.into(),
            proxy_settings: *proxy_settings,
            jr,
            views: Vec::new(),
        }
    }

    pub fn emit(&self) -> ProxyTemplate {
        let direct_fns = self
            .jr
            .direct_functions
            .values()
            .map(|f| self.emit_direct_function(f))
            .collect::<Vec<_>>();

        let deferred_fns = self
            .jr
            .deferred_functions
            .values()
            .map(|(id, f)| self.emit_deferred_function(f, *id))
            .collect::<Vec<_>>();

        ProxyTemplate {
            package: self.package.clone(),
            jxt_package: self.jxt_package.clone(),
            jextract_class_name: self.jextract_class_name.clone(),
            proxy_class_name: self.proxy_class_name.clone(),
            deferred_fns,
            direct_fns,
            proxy_settings: self.proxy_settings,
            has_scratchpad_overloads: false,
            scratchpad_layouts_byte_sizes: "".to_string(),
        }
    }

    #[inline]
    fn flatten_params(&self, params: &[IrParameter]) -> Vec<FlatParam> {
        let mut out = Vec::new();
        for param in params {
            self.flatten_type(&param.ty, &param.name, "", &mut out);
        }
        out
    }

    fn flatten_type(
        &self,
        ir_type: &IrType,
        field_name: &str,
        prefix: &str,
        out: &mut Vec<FlatParam>,
    ) {
        let java_name = if prefix.is_empty() {
            to_camel_case(field_name)
        } else {
            format!("{}{}", to_camel_case(prefix), to_pascal_case(field_name))
        };

        match &ir_type.kind {
            IrTypeKind::Primitive(p) => {
                out.push(FlatParam {
                    java_name,
                    java_type: primitive_to_java_type(p).to_string(),
                });
            }
            IrTypeKind::Pointer { .. }
            | IrTypeKind::Reference { .. }
            | IrTypeKind::FixedArray { .. }
            | IrTypeKind::FunctionPointer { .. } => {
                out.push(FlatParam {
                    java_name,
                    java_type: "MemorySegment".to_string(),
                });
            }
            IrTypeKind::Named(name) => {
                if let Some(ir_enum) = self.jr.enums.get(name) {
                    out.push(FlatParam {
                        java_name,
                        java_type: primitive_to_java_type(&ir_enum.underlying_type).to_string(),
                    });
                    return;
                }

                let s = self
                    .jr
                    .structs
                    .get(name)
                    .expect("unknown struct mapping encountered");

                let next_prefix = if prefix.is_empty() {
                    field_name.to_string()
                } else {
                    format!("{}_{}", prefix, field_name)
                };

                for child in &s.fields {
                    self.flatten_type(&child.ty, &child.name, &next_prefix, out);
                }
            }
        }
    }

    fn emit_direct_function(&self, function: &IrFunction) -> TemplateFn {
        let flat_params = self.flatten_params(&function.params);
        let call_params = flat_params
            .iter()
            .map(|p| p.java_name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        let return_ty = self.jr.map_ir_type_kind_to_java(&function.return_type.kind);

        let signature = format_signature(&return_ty, &function.name, &flat_params);

        let call = format!(
            "{}.{}({})",
            self.jextract_class_name, &function.name, call_params
        );

        let mut w = Writer::new();

        match &function.return_type.kind {
            IrTypeKind::Primitive(PrimitiveType::Void) => w.line(&format!("{call};")),
            IrTypeKind::Primitive(_) => w.line(&format!("return {call};")),
            IrTypeKind::Named(name) if self.jr.enums.contains_key(name) => {
                w.line(&format!("return {call};"));
            }
            IrTypeKind::Named(_) => {
                panic!(
                    "User-defined types are not supported in direct function calls, use pointer instead."
                );
            }
            kind @ (IrTypeKind::Pointer { to, .. } | IrTypeKind::Reference { to }) => {
                let is_const = match kind {
                    IrTypeKind::Pointer { is_ptr_const, .. } => *is_ptr_const,
                    _ => true,
                };

                w.line(&format!("MemorySegment ptr = {call};"));
                w.indent();
                w.line("");
                w.line("if (ptr.equals(MemorySegment.NULL)) {");
                w.indent();
                w.line(&format!(
                    "throw new NullPointerException(\"{} returned a NULL pointer\");",
                    function.name
                ));
                w.dedent();
                w.line("}");
                w.line("");

                let layout = format!(
                    "{}.byteSize()",
                    self.jr.map_ir_type_kind_to_java_layout(&to.kind)
                );
                if is_const {
                    w.line(&format!(
                        "return ptr.reinterpret({layout}).asReadOnlySegment();"
                    ));
                } else {
                    w.line(&format!("return ptr.reinterpret({layout});"));
                }
            }
            IrTypeKind::FixedArray { element_type, size } => {
                w.line(&format!("MemorySegment ptr = {call};"));
                w.indent();
                w.line("");
                w.line("if (ptr.equals(MemorySegment.NULL)) {");
                w.indent();
                w.line(&format!(
                    "throw new NullPointerException(\"{} returned a NULL array pointer\");",
                    function.name
                ));
                w.dedent();
                w.line("}");
                w.line("");

                let element_layout = self.jr.map_ir_type_kind_to_java_layout(&element_type.kind);
                w.line(&format!(
                    "return ptr.reinterpret(({size}) * {element_layout}.byteSize());"
                ));
            }
            IrTypeKind::FunctionPointer { .. } => {
                w.line(&format!("MemorySegment ptr = {call};"));
                w.indent();
                w.line("");
                w.line("if (ptr.equals(MemorySegment.NULL)) {");
                w.indent();
                w.line(&format!(
                    "throw new NullPointerException(\"{} returned a NULL function pointer\");",
                    function.name
                ));
                w.dedent();
                w.line("}");
                w.line("");

                w.line("return ptr;");
            }
        }

        TemplateFn {
            signature,
            body: w.finish(),
        }
    }

    pub fn emit_buf_setters<T: Into<IrTypedVar> + Clone>(
        &self,
        params: &[T],
        parent_class: &str,
        offset_prefix: &str,
        name_prefix: &str,
        writer: &mut Writer,
    ) {
        params.iter().cloned().for_each(|field| {
            let field: IrTypedVar = field.into();

            let java_base = if name_prefix.is_empty() {
                to_camel_case(&field.name)
            } else {
                format!(
                    "{}{}",
                    to_camel_case(name_prefix),
                    to_pascal_case(&field.name)
                )
            };

            let mut write_layout = |layout: &str| {
                writer.line(&format!(
                    "threadState.buffer.set({}, payloadCursor + {}{}.{}$offset(), {});",
                    layout, offset_prefix, parent_class, field.name, java_base
                ));
            };

            match &field.ty.kind {
                IrTypeKind::Primitive(primitive) => {
                    write_layout(primitive_to_java_layout(primitive))
                }
                IrTypeKind::Pointer { .. }
                | IrTypeKind::Reference { .. }
                | IrTypeKind::FunctionPointer { .. } => write_layout("ValueLayout.ADDRESS"),
                IrTypeKind::FixedArray { element_type, size } => {
                    let element_layout_call = match &element_type.kind {
                        IrTypeKind::Primitive(p) => {
                            format!("{}.byteSize()", primitive_to_java_layout(p))
                        }
                        IrTypeKind::Named(name) => {
                            format!("{}.$LAYOUT.byteSize()", name)
                        }
                        _ => "ValueLayout.ADDRESS.byteSize()".to_string(),
                    };

                    write_layout(&format!("({}) * {}", size, element_layout_call));
                }

                // --- 4. User Defined Variants (Structs & Enums) ---
                IrTypeKind::Named(type_name) => {
                    if let Some(e) = self.jr.enums.get(type_name) {
                        write_layout(primitive_to_java_layout(&e.underlying_type));
                        return;
                    }

                    let s = self.jr.structs.get(type_name).unwrap();

                    let new_offset_prefix =
                        format!("{offset_prefix}{parent_class}.{}$offset() + ", field.name);
                    let new_name_prefix = if name_prefix.is_empty() {
                        field.name.clone()
                    } else {
                        format!("{}_{}", name_prefix, field.name)
                    };

                    self.emit_buf_setters(
                        s.fields.as_slice(),
                        type_name,
                        &new_offset_prefix,
                        &new_name_prefix,
                        writer,
                    );
                }
            }
        });
    }

    fn emit_deferred_function(&self, function: &IrFunction, cmd_id: u32) -> TemplateFn {
        let flat_params = self.flatten_params(&function.params);

        let return_ty = self.jr.map_ir_type_kind_to_java(&function.return_type.kind);

        let signature = format_signature(&return_ty, &function.name, &flat_params);

        let fn_struct = &format!("Func{}", capitalize(&to_camel_case(&function.name)));

        let mut w = Writer::new();

        w.line(&format!(
            "long payloadSize = {fn_struct}.layout().byteSize();"
        ));
        w.indent();
        w.indent();
        w.line(&format!(
            "long payloadCursor = this.reserveCommandSpace(threadState, payloadSize, {cmd_id});"
        ));

        self.emit_buf_setters(&function.params, fn_struct, "", "", &mut w);

        TemplateFn {
            signature,
            body: w.finish(),
        }
    }
}
