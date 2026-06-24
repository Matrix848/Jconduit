use crate::ir::{
    InterRepr, IrEnum, IrFunction, IrStruct, IrType, IrTypeDef, IrTypeKind, PrimitiveType,
};
use crate::utils::formatting::{screaming_snake_to_pascal_case, to_pascal_case};
use proc_macro2::Literal;
use quote::__private::TokenStream;
use quote::{format_ident, quote};

pub type StructTokens = TokenStream;
pub type CmdMatchArmTokens = TokenStream;
pub type FuncDecl = TokenStream;

// Use proc_macro2 instead of quote's private namespace

pub fn map_primitive_to_rust(p: &PrimitiveType) -> TokenStream {
    match p {
        PrimitiveType::Void => quote! { () },
        PrimitiveType::Bool => quote! { bool },
        PrimitiveType::I8 => quote! { i8 },
        PrimitiveType::U8 => quote! { u8 },
        PrimitiveType::I16 => quote! { i16 },
        PrimitiveType::U16 => quote! { u16 },
        PrimitiveType::I32 => quote! { i32 },
        PrimitiveType::U32 => quote! { u32 },
        PrimitiveType::I64 => quote! { i64 },
        PrimitiveType::U64 => quote! { u64 },
        PrimitiveType::Float => quote! { f32 },
        PrimitiveType::Double => quote! { f64 },
        PrimitiveType::SizeT | PrimitiveType::Uptr => quote! { usize },
        PrimitiveType::SsizeT | PrimitiveType::PtrDiffT | PrimitiveType::Iptr => quote! { isize },
        PrimitiveType::Wchar => quote! { u16 },
    }
}

fn ir_type_to_rust(ir_type: &IrType, in_struct_context: bool) -> TokenStream {
    let type_kind = &ir_type.kind;
    match type_kind {
        IrTypeKind::Primitive(p) => map_primitive_to_rust(p),
        IrTypeKind::Named(s) => {
            let ident = format_ident!("{}", s); // Name strings must still become Idents!
            quote! { #ident }
        }
        IrTypeKind::Reference { to } => {
            let inner = ir_type_to_rust(to, in_struct_context);
            if in_struct_context {
                if to.is_base_const {
                    quote! { *const #inner }
                } else {
                    quote! { *mut #inner }
                }
            } else {
                if to.is_base_const {
                    quote! { &#inner }
                } else {
                    quote! { &mut #inner }
                }
            }
        }
        IrTypeKind::Pointer { to, .. } => {
            let inner = ir_type_to_rust(to, in_struct_context);
            if to.is_base_const {
                quote! { *const #inner }
            } else {
                quote! { *mut #inner }
            }
        }
        IrTypeKind::FixedArray { element_type, size } => {
            let inner = ir_type_to_rust(element_type, in_struct_context);
            quote! { [#inner; #size] }
        }
        IrTypeKind::FunctionPointer {
            return_type,
            arguments,
        } => {
            let args = arguments
                .iter()
                .map(|arg| ir_type_to_rust(arg, in_struct_context));
            let ret = ir_type_to_rust(return_type, in_struct_context);
            quote! { Option<unsafe extern "C" fn(#(#args),*) -> #ret> }
        }
    }
}

fn process_structs(ir_struct: &IrStruct) -> StructTokens {
    let struct_ident = format_ident!("{}", ir_struct.name);
    let repr = ir_struct.alignment.map_or_else(
        || quote! { #[repr(C)] },
        |v| quote! { #[repr(C, align(#v))] },
    );

    if ir_struct.fields.is_empty() {
        return quote! {
            #repr
            #[derive(Copy, Clone)]
            pub struct #struct_ident {
                _opaque: [u8; 0],
                _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
            }
        };
    }

    let fields = ir_struct.fields.iter().map(|field| {
        let field_name = format_ident!("{}", field.name);
        let field_ty = ir_type_to_rust(&field.ty, true);
        quote! {
            #field_name: #field_ty
        }
    });

    quote! {
        #repr
        #[derive(Copy, Clone)]
        pub struct #struct_ident {
            #( pub #fields,)*
        }
    }
}

fn process_enum(ir_enum: &IrEnum) -> TokenStream {
    let enum_ident = format_ident!("{}", &ir_enum.name);
    let variants = ir_enum.variants.iter().map(|variant| {
        let ident = format_ident!("{}", screaming_snake_to_pascal_case(&variant.name));

        let value = Literal::i64_unsuffixed(variant.value);
        quote! {
            #ident = #value
        }
    });

    let repr = if ir_enum.underlying_type == PrimitiveType::I32 {
        quote! {
            #[repr(C)]
            #[derive(Copy, Clone)]
        }
    } else {
        let und_ty = map_primitive_to_rust(&ir_enum.underlying_type);
        quote! {
            #[repr(#und_ty)]
        }
    };

    quote! {
        #repr
        pub enum #enum_ident {
            #(#variants),*
        }
    }
}

fn process_typedef(ir_typedef: &IrTypeDef) -> TokenStream {
    let typedef_ident = format_ident!("{}", &ir_typedef.name);
    let target_ident = ir_type_to_rust(&ir_typedef.target, false);

    quote! {
        pub type #typedef_ident = #target_ident;
    }
}

fn process_functions(ir_fn: &IrFunction, id: u32) -> (CmdMatchArmTokens, StructTokens, FuncDecl) {
    let fn_struct_name = format_ident!("Func{}", to_pascal_case(&ir_fn.name));
    let fn_ident = format_ident!("{}", ir_fn.name);

    let (struct_fields, fn_inputs) = ir_fn
        .params
        .iter()
        .map(|param| {
            let param_name = format_ident!("{}", param.name);
            let param_ty = ir_type_to_rust(&param.ty, false);
            (
                quote! {
                    #param_name: #param_ty
                },
                quote! {
                    #param_name
                },
            )
        })
        .collect::<(Vec<_>, Vec<_>)>();

    let arm_tokens = if !fn_inputs.is_empty() {
        quote! {
            #id => {
                unsafe {
                    let ptr = payload_ptr as *const #fn_struct_name;
                    let cmd = &*ptr;

                    #fn_ident( #(cmd.#fn_inputs),* );
                    size_of::<#fn_struct_name>()
                }
            }
        }
    } else {
        quote! {
            #id => {
                unsafe {
                    #fn_ident();
                    0
                }
            }
        }
    };

    let struct_tokens = quote! {
        #[repr(C)]
        pub struct #fn_struct_name {
            #(#struct_fields,)*
        }
    };

    let func_decl = quote! {
        pub fn #fn_ident( #( #struct_fields ),* );
    };

    (arm_tokens, struct_tokens, func_decl)
}

pub fn gen_tokens(ir: &InterRepr) -> TokenStream {
    let ir_enum = ir.enums.values().map(process_enum).collect::<Vec<_>>();

    let ir_typedef = ir
        .typedefs
        .values()
        .map(process_typedef)
        .collect::<Vec<_>>();

    let ir_structs = ir.structs.values().map(process_structs).collect::<Vec<_>>();

    let (match_arms_tokens, fn_structs_tokens, extern_decls) = ir
        .deferred_functions
        .values()
        .map(|(id, f)| process_functions(f, *id))
        .collect::<(Vec<_>, Vec<_>, Vec<_>)>();

    let version = env!("CARGO_PKG_VERSION");
    let comment_string = format!(
        " Automatically generated by Jconduit v{}. Changes will be overwritten if the file is regenerated.",
        version
    );

    quote! {
        #![allow(clippy::too_many_arguments)]
        #[doc=#comment_string]

        #(#ir_enum)*

        #(#ir_typedef)*

        #(#ir_structs)*

        #(#fn_structs_tokens)*

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn flush_buffer(buffer_ptr: *mut u8) {
            if buffer_ptr.is_null() { return; }

            let command_count = unsafe { *(buffer_ptr as *const u32) };
            if command_count == 0 { return; }

            let mut cursor = unsafe{buffer_ptr.add(4)};

            for _ in 0..command_count {
                // 1. Calculate padding
                let padding_required = (-(cursor as isize + 4) as usize) & 15usize;
                // 2. Advance the cursor
                cursor = unsafe { cursor.add(padding_required) };
                // 3. Read the command id
                let command_id = unsafe { *(cursor as *const u32) };
                // 4. Advance the cursor to the payload
                cursor = unsafe { cursor.add(4) };
                // 5. Dispatch the command
                let payload_size = dispatch_command(cursor, command_id);
                // 6. Advance the cursor by the command payload size
                cursor = unsafe { cursor.add(payload_size) };
            }
        }

         fn dispatch_command(payload_ptr: *const u8, command_id: u32) -> usize {
            match command_id {
                #(#match_arms_tokens)*
                _ => panic!("No command with id: {}", command_id),
            }
        }

        unsafe extern "C" {
            #(#extern_decls)*
        }
    }
}
