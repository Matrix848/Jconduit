use crate::generator::ForeignFunctions;
use crate::utils::jconduit_callbacks::PrefixStripper;
use crate::GeneratorContext;
use std::path::Path;

fn bindgen_ffi_module(ctx: &GeneratorContext, output_file: &Path) -> anyhow::Result<()> {
    let mut builder = bindgen::Builder::default()
        .header(ctx.functions_header.to_string_lossy())
        .layout_tests(ctx.layout_tests)
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        });

    // 1. Allowlists
    for p in &ctx.allowlist_functions {
        builder = builder.allowlist_function(p);
    }
    for p in &ctx.allowlist_types {
        builder = builder.allowlist_type(p);
    }
    for p in &ctx.allowlist_vars {
        builder = builder.allowlist_var(p);
    }
    for p in &ctx.allowlist_items {
        builder = builder.allowlist_item(p);
    }
    for f in &ctx.allowlist_files {
        builder = builder.allowlist_file(f);
    }

    // 2. Blocklists
    for p in &ctx.blocklist_functions {
        builder = builder.blocklist_function(p);
    }
    for p in &ctx.blocklist_types {
        builder = builder.blocklist_type(p);
    }
    for p in &ctx.blocklist_vars {
        builder = builder.blocklist_var(p);
    }
    for p in &ctx.blocklist_items {
        builder = builder.blocklist_item(p);
    }
    for f in &ctx.blocklist_files {
        builder = builder.blocklist_file(f);
    }

    // 3. Trait Optimization Modifiers
    for p in &ctx.opaque_types {
        builder = builder.opaque_type(p);
    }
    for p in &ctx.no_copy_types {
        builder = builder.no_copy(p);
    }
    for p in &ctx.no_debug_types {
        builder = builder.no_debug(p);
    }
    for p in &ctx.no_default_types {
        builder = builder.no_default(p);
    }
    for p in &ctx.no_hash_types {
        builder = builder.no_hash(p);
    }

    for file in &ctx.blocklist_files {
        builder = builder.blocklist_file(file);
    }

    for line in &ctx.raw_lines {
        builder = builder.raw_line(line);
    }

    builder = builder
        .derive_copy(ctx.derive_copy)
        .derive_default(ctx.derive_default);

    if ctx.use_core {
        builder = builder.use_core();
    }

    if !ctx.output_dir.exists() {
        std::fs::create_dir_all(&ctx.output_dir)?;
    }

    builder
        .parse_callbacks(Box::new(PrefixStripper::new(ctx.strip_prefix.clone())))
        .generate()?
        .write_to_file(ctx.output_dir.join(output_file))?;

    Ok(())
}

fn type_is_primitive(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => {
            let ident = path.path.segments.last().unwrap().ident.to_string();
            matches!(
                ident.as_str(),
                "u8" | "u16"
                    | "u32"
                    | "u64"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "f32"
                    | "f64"
                    | "bool"
            )
        }
        _ => false,
    }
}

fn parse_ffi(ctx: &GeneratorContext, ffi_path: &Path) -> anyhow::Result<ForeignFunctions> {
    let ffi_mod_path = ctx.output_dir.join(ffi_path);
    let ffi_src = std::fs::read_to_string(ffi_mod_path)?;
    let ffi_parser = syn::parse_file(&ffi_src)?;

    let mut ff_signatures = ForeignFunctions::default();

    let mut deferred_count = 0;

    for item in ffi_parser.items {
        if let syn::Item::ForeignMod(foreign_mod) = item {
            for foreign_item in foreign_mod.items {
                if let syn::ForeignItem::Fn(fn_item) = foreign_item {
                    match &fn_item.sig.output {
                        syn::ReturnType::Type(_, ty) => {
                            if type_is_primitive(ty) {
                                ff_signatures.direct.push(fn_item.sig.clone());
                                continue;
                            } else if let syn::Type::Ptr(_) = **ty {
                                ff_signatures.direct.push(fn_item.sig.clone());
                                continue;
                            }
                            panic!(
                                "ERROR: function signature {:?} has an unsupported return type. Only raw pointers and primitives are allowed.",
                                fn_item.sig.ident.to_string()
                            );
                        }
                        syn::ReturnType::Default => {
                            let is_direct = |value: String| -> bool {
                                ctx.direct_prefixes.iter().any(|kw| value.starts_with(kw))
                                    || ctx.direct_keywords.iter().any(|kw| value.contains(kw))
                            };

                            if is_direct(fn_item.sig.ident.to_string()) {
                                let has_output = |value: String| -> bool {
                                    ctx.output_param_prefix
                                        .iter()
                                        .any(|kw| value.starts_with(kw))
                                        || ctx
                                            .output_param_suffix
                                            .iter()
                                            .any(|kw| value.ends_with(kw))
                                };
                                let inputs_iter = fn_item.sig.inputs.iter().enumerate();

                                for (i, arg) in inputs_iter {
                                    if let syn::FnArg::Typed(syn::PatType { pat, .. }) = arg
                                        && let syn::Pat::Ident(syn::PatIdent { ident, .. }) = &**pat
                                        && has_output(ident.to_string())
                                    {
                                        if i == fn_item.sig.inputs.len() - 1 {
                                            ff_signatures
                                                .direct_scratchpad
                                                .push(fn_item.sig.clone());
                                        } else {
                                            panic!(
                                                "ERROR: function signature {:?} has more than one output parameter. Only one output parameter is allowed.",
                                                fn_item.sig.ident.to_string()
                                            );
                                        }
                                    }
                                }
                                ff_signatures.direct.push(fn_item.sig.clone());
                            } else {
                                ff_signatures
                                    .deferred
                                    .push((deferred_count, fn_item.sig.clone()));
                                deferred_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ff_signatures)
}

pub(super) fn generate_ffi(
    ctx: &GeneratorContext,
    ffi_file: &Path,
) -> anyhow::Result<ForeignFunctions> {
    bindgen_ffi_module(ctx, ffi_file)?;
    parse_ffi(ctx, ffi_file)
}
