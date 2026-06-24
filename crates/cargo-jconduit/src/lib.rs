pub mod generator {
    pub mod java;
    pub mod jextract;
    pub mod rust;
}
pub mod utils {
    pub mod formatting;
}

pub mod compiler;
pub mod ir;
pub mod parser;

use crate::generator::java::java_gen;
use crate::generator::jextract::gen_jxt_bindings;
use crate::generator::rust::gen_tokens;
use crate::ir::Lowerer;
use crate::parser::{HeaderParser, preprocess_header};
use crate::utils::formatting::to_pascal_case;
use anyhow::Result;
use clap::{Parser, Subcommand};
use indoc::indoc;
use log::warn;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "jconduit", version, about = "Java FFM Code Generator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Generate(GenerateArgs),
}

#[derive(Parser)]
pub struct GenerateArgs {
    #[arg(short, long, default_value = "jconduit.toml")]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct TomlConfig {
    pub generator: GeneratorSection,
    #[serde(default)]
    pub proxy_settings: ProxySettings,
}

pub fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Generate(args) => {
            let (toml_config, base_dir) = load_config(args.config)?;
            let ctx: GeneratorContext = GeneratorContext::from(toml_config, base_dir)?;
            generate_proxy(&ctx)?;
        }
    }
    Ok(())
}

fn generate_proxy(ctx: &GeneratorContext) -> Result<()> {
    let out_dir = ctx.output_dir.join("jconduit");
    fs::create_dir_all(out_dir.join("java"))?;
    fs::create_dir_all(out_dir.join("rust/src"))?;
    let java_dir = out_dir.join("java");

    let pre_processed_header = preprocess_header(&ctx.source_header)?;

    let parser = HeaderParser::new(&pre_processed_header, include_str!("query.scm"));
    let ir = Lowerer::new().lower(&parser.parse()?)?;

    let cc_name = to_pascal_case(&ctx.name);

    // Jextract bindings
    let jxt_class_name = format!("{}Jextract", cc_name);
    let jxt_package = format!("{}.jextract", &ctx.package);
    gen_jxt_bindings(&ir, &jxt_package, &jxt_class_name, &java_dir)?;

    let crate_name = ctx.name.replace("_", "-");

    // Generate Cargo.toml
    let dispatch_toml = format!(
        indoc! {r#"
            [package]
            name = "{0}"
            version = "{1}"
            edition = "2024"

            [lib]
            name = "{2}"
            crate-type = ["cdylib"]
        "#},
        crate_name, ctx.version, ctx.name
    );
    fs::write(out_dir.join("rust/Cargo.toml"), dispatch_toml)?;

    // Generate the Rust dispatcher
    let dispatch_tokens = gen_tokens(&ir);
    let dispatch = syn::parse2::<syn::File>(dispatch_tokens)?;
    let fmt_dispatch = prettyplease::unparse(&dispatch);
    fs::write(out_dir.join("rust/src/lib.rs"), fmt_dispatch)?;

    // Generate the Java classes
    let jcd_package = format!("{}.jconduit", ctx.package);
    java_gen(
        &java_dir,
        &jcd_package,
        &jxt_package,
        &jxt_class_name,
        &cc_name,
        &ctx.proxy_settings,
        &ir,
    )?;
    Ok(())
}

fn load_config(config_path: PathBuf) -> Result<(TomlConfig, PathBuf)> {
    let absolute_config_path = config_path
        .canonicalize()
        .expect("Failed to find absolute path of config file");

    let base_dir = absolute_config_path
        .parent()
        .expect("Failed to get config file directory")
        .to_path_buf();

    let toml_content =
        fs::read_to_string(&absolute_config_path).expect("Failed to read config file");
    let config: TomlConfig = toml::from_str(&toml_content)?;
    Ok((config, base_dir))
}

impl GeneratorContext {
    pub fn from(value: TomlConfig, base_dir: PathBuf) -> Result<GeneratorContext> {
        let anchor_path = |input_path: &str| -> PathBuf {
            let path = Path::new(input_path);
            let full_path = if path.is_absolute() {
                PathBuf::from(input_path)
            } else {
                base_dir.join(path)
            };
            dunce::simplified(&full_path).to_path_buf()
        };

        if value.proxy_settings.min_buffer_size > value.proxy_settings.max_buffer_size {
            anyhow::bail!(
                "ERROR: Min buffer size set to a value greater than max buffer size: {} > {}.",
                value.proxy_settings.min_buffer_size,
                value.proxy_settings.max_buffer_size
            );
        }

        if value.proxy_settings.max_buffer_size < 16 {
            anyhow::bail!(
                "ERROR: max_buffer_size must be greater than 16: {}",
                value.proxy_settings.max_buffer_size
            );
        }

        if value.proxy_settings.decaying_flushes_threshold < 1 {
            anyhow::bail!(
                "ERROR: Decaying frames threshold must be greater than 0: {}",
                value.proxy_settings.decaying_flushes_threshold
            );
        } else if value.proxy_settings.decaying_flushes_threshold > 3000 {
            warn!(
                "Decaying frames threshold is too high, it may cause performance issues. \
                 Consider setting it to a lower value (e.g. 300)."
            )
        }

        if value.proxy_settings.decaying_usage_threshold < 0.0 {
            anyhow::bail!(
                "ERROR: Decaying usage threshold must be greater than 0.0: {}",
                value.proxy_settings.decaying_usage_threshold
            );
        } else if value.proxy_settings.decaying_usage_threshold > 1.0 {
            anyhow::bail!(
                "ERROR: Decaying usage threshold must be less or equal to 1.0: {}",
                value.proxy_settings.decaying_usage_threshold
            );
        }

        if value.proxy_settings.shrink_rate < 0.0 {
            anyhow::bail!(
                "ERROR: Shrink rate must be greater than 0.0: {}",
                value.proxy_settings.shrink_rate
            );
        } else if value.proxy_settings.shrink_rate > 1.0 {
            anyhow::bail!(
                "ERROR: Shrink rate must be less or equal to 1.0: {}",
                value.proxy_settings.shrink_rate
            );
        }

        if value.proxy_settings.shrink_rate < value.proxy_settings.decaying_usage_threshold {
            anyhow::bail!(
                "ERROR: Decaying usage threshold must be less than or equal to shrink rate: {} <= {}. \
                        It will cause serious performance issues.",
                value.proxy_settings.shrink_rate,
                value.proxy_settings.decaying_usage_threshold
            );
        }

        if value.proxy_settings.growth_rate < 1.0 {
            anyhow::bail!(
                "ERROR: Growth rate must be greater than 1.0: {}",
                value.proxy_settings.growth_rate
            );
        } else if value.proxy_settings.growth_rate > 2.0 {
            warn!(
                "Growth rate is too high, it may cause performance issues. \
                 Consider setting it to a value lower then 2.0."
            )
        }

        if value.proxy_settings.auto_arena {
            warn!(
                "Using automatic arena allocation. This may cause performance issues. \
                 Consider setting it to false and handling closing it manually if you have \
                 explicit control over the lifecycle and termination of the threads pushing \
                 the commands."
            )
        }

        Ok(GeneratorContext {
            name: value.generator.name,
            package: value.generator.package,
            source_header: anchor_path(&value.generator.source_header),
            output_dir: anchor_path(&value.generator.output_dir),
            version: value.generator.version,

            // Settings
            proxy_settings: ProxySettings {
                min_buffer_size: value.proxy_settings.min_buffer_size,
                max_buffer_size: value.proxy_settings.max_buffer_size,
                decaying_flushes_threshold: value.proxy_settings.decaying_flushes_threshold,
                decaying_usage_threshold: value.proxy_settings.decaying_usage_threshold,
                shrink_rate: value.proxy_settings.shrink_rate,
                growth_rate: value.proxy_settings.growth_rate,
                auto_arena: value.proxy_settings.auto_arena,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct GeneratorSection {
    pub name: String,
    pub version: String,
    pub package: String,
    pub source_header: String,
    #[serde(default = "default_out_dir")]
    pub output_dir: String,
}
fn default_out_dir() -> String {
    "./generated".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneratorSettings {
    pub min_buffer_size: usize,
    pub max_buffer_size: usize,
    pub decaying_flushes_threshold: usize,
    pub decaying_usage_threshold: f32,
    pub shrink_rate: f32,
    pub growth_rate: f32,
    pub auto_arena: bool,
}

impl Default for GeneratorSettings {
    fn default() -> Self {
        Self {
            min_buffer_size: 64 * 1024,
            max_buffer_size: 2 * 1024 * 1024,
            decaying_flushes_threshold: 300,
            decaying_usage_threshold: 0.5,
            shrink_rate: 0.75,
            growth_rate: 1.50,
            auto_arena: true,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ProxySettings {
    pub min_buffer_size: usize,
    pub max_buffer_size: usize,
    pub decaying_flushes_threshold: usize,
    pub decaying_usage_threshold: f32,
    pub shrink_rate: f32,
    pub growth_rate: f32,
    pub auto_arena: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            min_buffer_size: 64 * 1024,
            max_buffer_size: 2 * 1024 * 1024,
            decaying_flushes_threshold: 300,
            decaying_usage_threshold: 0.5,
            shrink_rate: 0.75,
            growth_rate: 1.5,
            auto_arena: true,
        }
    }
}

pub struct GeneratorContext {
    name: String,
    version: String,
    package: String,
    source_header: PathBuf,
    output_dir: PathBuf,
    proxy_settings: ProxySettings,
}
