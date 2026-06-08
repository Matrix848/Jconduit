pub mod generator;
pub(crate) mod utils {
    pub(crate) mod formatting;
    pub(crate) mod jconduit_callbacks;
}

use crate::generator::generate_conduit;
use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Deserialize;
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
    pub filters: FilterSection,
    #[serde(default)]
    pub options: GeneratorOptions,
    #[serde(default)]
    pub proxy_settings: ProxySettings,
}

pub fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Generate(args) => {
            let (toml_config, base_dir) = load_config(args.config)?;
            let ctx: GeneratorContext = GeneratorContext::from(toml_config, base_dir)?;
            generate_conduit(&ctx)?;
        }
    }
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
        std::fs::read_to_string(&absolute_config_path).expect("Failed to read config file");
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

        let mut allowlist_item = value.filters.allowlist_functions.clone();
        allowlist_item.push(format!("^{}.*", value.generator.prefix));

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

        if value.proxy_settings.decaying_frames_threshold < 1 {
            anyhow::bail!(
                "ERROR: Decaying frames threshold must be greater than 0: {}",
                value.proxy_settings.decaying_frames_threshold
            );
        } else if value.proxy_settings.decaying_frames_threshold > 3000 {
            println!(
                "WARNING: Decaying frames threshold is too high, it may cause performance issues. \
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
            println!(
                "WARNING: Growth rate is too high, it may cause performance issues. \
                          Consider setting it to a value lower then 2.0."
            )
        }

        if value.proxy_settings.auto_arena {
            println!(
                "WARNING: Using automatic arena allocation. This may cause performance issues. \
                         Consider setting it to false and handling closing it manually if you have \
                         explicit control over the lifecycle and termination of the threads pushing \
                         the commands."
            )
        }

        Ok(GeneratorContext {
            crate_name: value.generator.crate_name,
            proxy_class_name: value.generator.proxy_class_name,
            package: value.generator.package,
            functions_header: anchor_path(&value.generator.functions_header),
            typedef_header: anchor_path(&value.generator.typedef_header),
            prefix: value.generator.prefix,
            output_dir: anchor_path(&value.generator.output_dir),
            version: value.generator.version,

            // 💡 All Allowlists
            allowlist_functions: value.filters.allowlist_functions,
            allowlist_types: value.filters.allowlist_types,
            allowlist_vars: value.filters.allowlist_vars,
            allowlist_items: allowlist_item,
            allowlist_files: value.filters.allowlist_files,

            // 💡 All Blocklists
            blocklist_functions: value.filters.blocklist_functions,
            blocklist_types: value.filters.blocklist_types,
            blocklist_vars: value.filters.blocklist_vars,
            blocklist_items: value.filters.blocklist_items,
            blocklist_files: value.filters.blocklist_files,

            // 💡 Specialized Bindgen Modifiers
            opaque_types: value.filters.opaque_types,
            no_copy_types: value.filters.no_copy_types,
            no_debug_types: value.filters.no_debug_types,
            no_default_types: value.filters.no_default_types,
            no_hash_types: value.filters.no_hash_types,

            // Settings
            proxy_settings: ProxySettings {
                min_buffer_size: value.proxy_settings.min_buffer_size,
                max_buffer_size: value.proxy_settings.max_buffer_size,
                decaying_frames_threshold: value.proxy_settings.decaying_frames_threshold,
                decaying_usage_threshold: value.proxy_settings.decaying_usage_threshold,
                shrink_rate: value.proxy_settings.shrink_rate,
                growth_rate: value.proxy_settings.growth_rate,
                auto_arena: value.proxy_settings.auto_arena,
            },

            // Codegen Options
            strip_prefix: value.options.strip_prefix,
            derive_copy: value.options.derive_copy,
            derive_default: value.options.derive_default,
            raw_lines: value.options.raw_lines,
            use_core: value.options.use_core,
            c_naming_conversion: value.options.c_naming_conversion,
            layout_tests: value.options.layout_tests,
            direct_prefixes: value.options.direct_prefixes,
            direct_keywords: value.options.direct_keywords,
            output_param_prefix: value.options.output_param_prefix,
            output_param_suffix: value.options.output_param_suffix,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct GeneratorSection {
    pub crate_name: String,
    pub proxy_class_name: String,
    pub package: String,
    pub functions_header: String,
    pub typedef_header: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "default_out_dir")]
    pub output_dir: String,
}
fn default_version() -> String {
    "0.1.0".to_string()
}
fn default_out_dir() -> String {
    "./generated/jconduit".to_string()
}

#[derive(Debug, Deserialize)]
#[serde(default)] // 💡 Allows users to cherry-pick partial fields in TOML
pub struct FilterSection {
    pub allowlist_functions: Vec<String>,
    pub allowlist_types: Vec<String>,
    pub allowlist_vars: Vec<String>,
    pub allowlist_items: Vec<String>,
    pub allowlist_files: Vec<String>,
    pub blocklist_functions: Vec<String>,
    pub blocklist_types: Vec<String>,
    pub blocklist_vars: Vec<String>,
    pub blocklist_items: Vec<String>,
    pub blocklist_files: Vec<String>,
    pub opaque_types: Vec<String>,
    pub no_copy_types: Vec<String>,
    pub no_debug_types: Vec<String>,
    pub no_default_types: Vec<String>,
    pub no_hash_types: Vec<String>,
}

impl Default for FilterSection {
    fn default() -> Self {
        Self {
            allowlist_functions: Vec::new(),
            allowlist_types: Vec::new(),
            allowlist_vars: Vec::new(),
            allowlist_items: Vec::new(),
            allowlist_files: vec![],
            blocklist_functions: Vec::new(),
            blocklist_types: vec!["^__.*".to_string(), "^va_list.*".to_string()],
            blocklist_vars: vec!["^__.*".to_string(), "^COBJMACROS$".to_string()],
            blocklist_items: vec!["^__.*".to_string()],
            blocklist_files: Vec::new(),
            opaque_types: vec!["^opaque_.*".to_string()],
            no_copy_types: Vec::new(),
            no_debug_types: Vec::new(),
            no_default_types: Vec::new(),
            no_hash_types: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneratorSettings {
    pub min_buffer_size: usize,
    pub max_buffer_size: usize,
    pub decaying_frames_threshold: usize,
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
            decaying_frames_threshold: 300,
            decaying_usage_threshold: 0.5,
            shrink_rate: 0.75,
            growth_rate: 1.50,
            auto_arena: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneratorOptions {
    pub strip_prefix: String,
    pub derive_copy: bool,
    pub derive_default: bool,
    pub use_core: bool,
    pub c_naming_conversion: bool,
    pub raw_lines: Vec<String>,
    pub layout_tests: bool,
    pub direct_prefixes: Vec<String>,
    pub direct_keywords: Vec<String>,
    pub output_param_prefix: Vec<String>,
    pub output_param_suffix: Vec<String>,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            strip_prefix: "".to_string(),
            derive_copy: true,
            derive_default: false,
            use_core: false ,
            c_naming_conversion: false,
            raw_lines: vec!["#![allow(dead_code, non_camel_case_types, non_snake_case, non_upper_case_globals)]".to_string()],
            layout_tests: true,
            direct_prefixes: vec!["get_".to_string(), "fetch_".to_string(), "jconduit_direct_".to_string()],
            direct_keywords: vec!["_get_".to_string()],
            output_param_prefix: vec![],
            output_param_suffix: vec!["_out".to_string(), "_dest".to_string()],
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ProxySettings {
    pub min_buffer_size: usize,
    pub max_buffer_size: usize,
    pub decaying_frames_threshold: usize,
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
            decaying_frames_threshold: 300,
            decaying_usage_threshold: 0.5,
            shrink_rate: 0.75,
            growth_rate: 1.5,
            auto_arena: true,
        }
    }
}

pub struct GeneratorContext {
    pub crate_name: String,
    pub proxy_class_name: String,
    pub package: String,
    pub functions_header: PathBuf,
    pub typedef_header: PathBuf,
    pub prefix: String,
    pub output_dir: PathBuf,
    pub version: String,
    pub allowlist_functions: Vec<String>,
    pub allowlist_types: Vec<String>,
    pub allowlist_vars: Vec<String>,
    pub allowlist_items: Vec<String>,
    pub allowlist_files: Vec<String>,
    pub blocklist_functions: Vec<String>,
    pub blocklist_types: Vec<String>,
    pub blocklist_vars: Vec<String>,
    pub blocklist_items: Vec<String>,
    pub blocklist_files: Vec<String>,
    pub opaque_types: Vec<String>,
    pub no_copy_types: Vec<String>,
    pub no_debug_types: Vec<String>,
    pub no_default_types: Vec<String>,
    pub no_hash_types: Vec<String>,
    pub proxy_settings: ProxySettings,
    pub strip_prefix: String,
    pub derive_copy: bool,
    pub derive_default: bool,
    pub use_core: bool,
    pub c_naming_conversion: bool,
    pub raw_lines: Vec<String>,
    pub layout_tests: bool,
    pub direct_prefixes: Vec<String>,
    pub direct_keywords: Vec<String>,
    pub output_param_prefix: Vec<String>,
    pub output_param_suffix: Vec<String>,
}
