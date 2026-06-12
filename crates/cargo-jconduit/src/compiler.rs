// ------------------ Compiler ------------------

use log::info;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CompilerError {
    #[error("Could not read header: {path}")]
    HeaderReadFailed { path: PathBuf },
    #[error("C++ syntax verification failed for header: {path}")]
    SyntaxCheckFailed {
        path: PathBuf,
        exit_code: Option<i32>,
        output: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compiler {
    Clang,
    Gcc,
}

impl Compiler {
    pub fn get() -> &'static Compiler {
        static COMPILER: OnceLock<Compiler> = OnceLock::new();
        COMPILER.get_or_init(Self::find_compiler)
    }
    fn find_compiler() -> Self {
        let available = |name: &str| {
            Command::new(name)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok()
        };

        if available(Self::Clang.as_str()) {
            Self::Clang
        } else if available(Self::Gcc.as_str()) {
            Self::Gcc
        } else {
            panic!("Could not find a supported C++ compiler. Please install clang or gcc.");
        }
    }

    pub fn command(&self) -> Command {
        info!("Using compiler: {}", self.as_str());
        Command::new(self.as_str())
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Clang => "clang++",
            Self::Gcc => "g++",
        }
    }

    pub fn verify_header(&self, header: &Path) -> Result<(), CompilerError> {
        let output = self
            .command()
            .args(["-fsyntax-only", "-std=c++20"])
            .arg(header)
            .output()
            .expect("compiler was verified at startup but failed to execute");

        if !output.status.success() {
            return Err(CompilerError::SyntaxCheckFailed {
                path: header.to_path_buf(),
                exit_code: output.status.code(),
                output: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        Ok(())
    }
}
