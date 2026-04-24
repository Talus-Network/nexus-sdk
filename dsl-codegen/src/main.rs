//! `nexus-dsl-codegen` CLI — read `tool-meta.json` artifacts, emit Rust
//! and TypeScript DSL descriptors.

use {
    anyhow::{Context, Result},
    clap::Parser,
    nexus_dsl_codegen::{emit_rust, emit_ts, load_descriptor},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

#[derive(Parser, Debug)]
#[command(
    name = "nexus-dsl-codegen",
    about = "Emit Rust + TypeScript DSL descriptors from tool-meta.json"
)]
struct Cli {
    /// One or more `tool-meta.json` files to process.
    #[arg(short, long, required = true, num_args = 1..)]
    meta: Vec<PathBuf>,

    /// Directory to write Rust descriptor modules into. One file per tool:
    /// `<name>.rs`.
    #[arg(long)]
    rust_out: Option<PathBuf>,

    /// Directory to write TypeScript descriptor modules into. One file
    /// per tool: `<name>.ts`.
    #[arg(long)]
    ts_out: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.rust_out.is_none() && cli.ts_out.is_none() {
        anyhow::bail!("at least one of --rust-out / --ts-out must be provided");
    }

    for meta_path in &cli.meta {
        let desc = load_descriptor(meta_path)
            .with_context(|| format!("loading descriptor from {}", meta_path.display()))?;

        if let Some(dir) = &cli.rust_out {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating Rust output dir {}", dir.display()))?;
            let out = dir.join(format!("{}.rs", rust_mod_name(&desc.type_name)));
            write_if_changed(&out, &emit_rust(&desc))?;
            eprintln!("wrote {}", out.display());
        }

        if let Some(dir) = &cli.ts_out {
            fs::create_dir_all(dir)
                .with_context(|| format!("creating TS output dir {}", dir.display()))?;
            let out = dir.join(format!("{}.ts", ts_mod_name(&desc.type_name)));
            write_if_changed(&out, &emit_ts(&desc))?;
            eprintln!("wrote {}", out.display());
        }
    }

    Ok(())
}

fn rust_mod_name(type_name: &str) -> String {
    // Lowercase the PascalCase type name to get a snake_case-ish module
    // name. Good enough for single-word tool names; multi-word will come
    // through as `adder`/`addr` etc., which is acceptable.
    type_name.to_lowercase()
}

fn ts_mod_name(type_name: &str) -> String {
    type_name.to_string()
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
