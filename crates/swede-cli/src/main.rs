//! `swede` — command-line front end: validate, render, schedule.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "swede", version, about = "Graph-first recipe language tooling")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse + validate a file, printing coded diagnostics.
    Validate {
        file: PathBuf,
        /// Emit diagnostics as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Render a recipe to a table.
    Render {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Flow)]
        format: Format,
    },
    /// Schedule a recipe or menu onto a single-cook timeline.
    Schedule { file: PathBuf },
    /// Dump the parsed AST as JSON (debugging).
    Parse { file: PathBuf },
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    /// Markdown flow table (one row per operation).
    Flow,
    /// Monospace depth-column grid.
    Grid,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Validate { file, json } => validate(file, json),
        Cmd::Render { file, format } => render(file, format),
        Cmd::Schedule { file } => schedule(file),
        Cmd::Parse { file } => parse(file),
    }
}

fn read(file: &PathBuf) -> String {
    std::fs::read_to_string(file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {e}", file.display());
        std::process::exit(2);
    })
}

fn validate(file: PathBuf, json: bool) -> ExitCode {
    let src = read(&file);
    let diags = swede_semantics::validate_source(&src);
    if json {
        println!("{}", serde_json::to_string_pretty(&diags).unwrap());
    } else if diags.is_empty() {
        println!("ok: {} — no diagnostics", file.display());
    } else {
        for d in &diags {
            println!("{}", swede_semantics::format_line(d));
        }
    }
    if diags.iter().any(|d| d.is_error()) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn render(file: PathBuf, format: Format) -> ExitCode {
    let src = read(&file);
    let lowered = swede_syntax::parse(&src);
    let diags = swede_semantics::validate(&lowered);
    for d in diags.iter().filter(|d| d.is_error()) {
        eprintln!("{}", swede_semantics::format_line(d));
    }
    match lowered.file {
        Some(swede_syntax::File::Recipe(r)) => {
            let out = match format {
                Format::Flow => swede_render::flow_table(&r),
                Format::Grid => swede_render::grid_table(&r),
            };
            print!("{out}");
            ExitCode::SUCCESS
        }
        Some(swede_syntax::File::Menu(_)) => {
            eprintln!("error: render expects a recipe, not a menu (try `schedule`)");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("error: could not parse {}", file.display());
            ExitCode::FAILURE
        }
    }
}

fn schedule(file: PathBuf) -> ExitCode {
    let src = read(&file);
    let lowered = swede_syntax::parse(&src);
    match lowered.file {
        Some(swede_syntax::File::Recipe(r)) => {
            let plan = swede_schedule::schedule_recipe(&r);
            print!("{}", swede_schedule::render_timeline(&plan));
            ExitCode::SUCCESS
        }
        Some(swede_syntax::File::Menu(m)) => {
            let dir = file.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            match swede_schedule::schedule_menu(&m, &dir) {
                Ok(plan) => {
                    print!("{}", swede_schedule::render_timeline(&plan));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        None => {
            eprintln!("error: could not parse {}", file.display());
            ExitCode::FAILURE
        }
    }
}

fn parse(file: PathBuf) -> ExitCode {
    let src = read(&file);
    let lowered = swede_syntax::parse(&src);
    match &lowered.file {
        Some(f) => {
            println!("{}", serde_json::to_string_pretty(f).unwrap());
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("error: could not parse {}", file.display());
            ExitCode::FAILURE
        }
    }
}
