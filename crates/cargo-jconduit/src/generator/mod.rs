use crate::generator::dispatcher_gen::{crate_gen, generate_dispatcher};
use crate::generator::ffi_gen::generate_ffi;
use crate::generator::java::{
    gen_proxy_bindings, generate_proxy_template, jextract_bindings, RustEnum, RustStruct,
};
use crate::GeneratorContext;
use anyhow::Result;
use askama::Template;
use std::fs;
use syn::Signature;

mod dispatcher_gen;
mod ffi_gen;
mod java;

#[derive(Debug, Default, Clone)]
pub struct ForeignFunctions {
    pub direct: Vec<Signature>,
    pub direct_scratchpad: Vec<Signature>,
    pub deferred: Vec<(u32, Signature)>,
}

pub(super) fn generate_conduit(ctx: &GeneratorContext) -> Result<()> {
    let root_dir = &ctx.output_dir;
    fs::create_dir_all(root_dir)?;

    // Generate the Rust library
    let crate_dir = &root_dir.join("rust");
    fs::create_dir_all(crate_dir)?;
    crate_gen(&ctx.crate_name, &ctx.version, crate_dir)?;
    let ffi_file = &crate_dir.join("src/ffi.rs");
    let ff = generate_ffi(ctx, ffi_file)?;
    let dispatcher_file = &crate_dir.join("src/dispatch.rs");
    generate_dispatcher(dispatcher_file, &ff)?;
    // Generate the Java library
    let java_dir = &root_dir.join("java");
    fs::create_dir_all(java_dir)?;
    let java_header_dir = &root_dir.join("include/");
    fs::create_dir_all(java_header_dir)?;
    let java_header_file = &java_header_dir.join("jconduit.h");

    let deferred_fn_names = ff
        .deferred
        .iter()
        .map(|(_, sig)| sig.ident.to_string())
        .collect::<Vec<_>>();

    gen_proxy_bindings(
        &ctx.typedef_header,
        &[dispatcher_file],
        deferred_fn_names,
        java_header_file,
    )?;

    let jextract_folder = &java_dir.join("ffm");
    let jextract_class_name = format!("{}Jextract", ctx.proxy_class_name);

    jextract_bindings(
        &ctx.package,
        &jextract_class_name,
        java_header_file,
        jextract_folder,
    )?;

    let file = fs::read_to_string(ffi_file)?;
    let parsed = syn::parse_file(&file)?;
    let structs_map = RustStruct::parse_all(&parsed);
    let enum_map = RustEnum::parse_all(&parsed);

    let proxy_template = generate_proxy_template(
        &ctx.package,
        &jextract_class_name,
        &ctx.proxy_class_name,
        &ctx.proxy_settings,
        ff,
        &structs_map,
        &enum_map,
    )?;

    let proxy_dir = &java_dir.join("proxy");
    fs::create_dir_all(proxy_dir)?;
    let proxy_file = &proxy_dir.join(ctx.proxy_class_name.clone() + ".java");
    fs::write(proxy_file, &proxy_template.render()?)?;

    Ok(())
}
