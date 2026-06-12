use crate::ir::{IrFunction, IrStruct, IrTypeKind, PrimitiveType};
use log::warn;
use quote::__private::TokenStream;
use quote::quote;

pub fn map_primitive_to_rust(p: &PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Void => "()",
        PrimitiveType::Bool => "bool",
        PrimitiveType::I8 => "i8",
        PrimitiveType::U8 => "u8",
        PrimitiveType::I16 => "i16",
        PrimitiveType::U16 => "u16",
        PrimitiveType::I32 => "i32",
        PrimitiveType::U32 => "u32",
        PrimitiveType::I64 => "i64",
        PrimitiveType::U64 => "u64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
        PrimitiveType::SizeT | PrimitiveType::Uptr => "usize",
        PrimitiveType::SsizeT | PrimitiveType::PtrDiffT | PrimitiveType::Iptr => "isize",
        PrimitiveType::Wchar => "u16",
    }
}

pub fn ir_type_to_rust(ir_type: &crate::ir::IrType, in_struct_context: bool) -> String {
    let type_kind = &ir_type.kind;
    match type_kind {
        IrTypeKind::Primitive(p) => map_primitive_to_rust(p).to_string(),
        IrTypeKind::UserDefined(s) => s.clone(),
        IrTypeKind::Reference { to } => {
            if in_struct_context {
                if to.is_base_const {
                    format!("*const {}", ir_type_to_rust(to, in_struct_context))
                } else {
                    format!("*mut {}", ir_type_to_rust(to, in_struct_context))
                }
            } else {
                if to.is_base_const {
                    format!("&{}", ir_type_to_rust(to, in_struct_context))
                } else {
                    format!("&mut {}", ir_type_to_rust(to, in_struct_context))
                }
            }
        }
        IrTypeKind::Pointer { to, .. } => format!(
            "*{} {}",
            if to.is_base_const { "const" } else { "mut" },
            ir_type_to_rust(to, in_struct_context),
        ),
        IrTypeKind::FixedArray { element_type, size } => {
            format!(
                "[{}; {}]",
                ir_type_to_rust(element_type, in_struct_context),
                size
            )
        }
        IrTypeKind::FunctionPointer {
            return_type,
            arguments,
        } => {
            let args = arguments
                .iter()
                .map(|arg| ir_type_to_rust(arg, in_struct_context))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Option<unsafe extern \"C\" fn({}) -> {}>",
                args,
                ir_type_to_rust(return_type, in_struct_context),
            )
        }
    }
}

pub fn generate_struct(ir_struct: IrStruct) -> TokenStream {
    let struct_name = ir_struct.name;
    let alignment = ir_struct.alignment;
    let is_vec = ir_struct.is_vec;

    if is_vec {
        warn!("jcd::vec is not supported yet, ignoring")
    }

    let fields = ir_struct.fields.iter().map(|field| {
        let field_name = &field.name;
        let field_ty = ir_type_to_rust(&field.ty, true);
        quote! {
            #field_name: #field_ty
        }
    });

    quote! {
        #[repr(C, align(#alignment))]
        pub struct #struct_name {
            #(#fields),*
        }
    }
}

pub fn generate_direct_fn(ir_fn: IrFunction) -> Option<TokenStream> {
    let fn_name = ir_fn.name;
    let return_type = ir_type_to_rust(&ir_fn.return_type, false);
    let params = ir_fn.params.iter().map(|param| {
        let param_name = &param.name;
        let param_ty = ir_type_to_rust(&param.ty, false);
        quote! {
            #param_name: #param_ty
        }
    });

    if ir_fn.out_handle.is_none() {
        return None;
    }

    None
}
