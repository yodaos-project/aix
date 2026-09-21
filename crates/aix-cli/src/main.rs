use aix_pack::{collector::CollectOptions, OptimizeOptions, PackOptions};
use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use uuid::Uuid;

mod launch;

#[derive(Parser)]
#[command(name = "aix")]
#[command(about = "Ink AIX Package Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect or change the device Developer Mode
    Device {
        /// Device action: set-dev or unset-dev; omit for a read-only status query
        #[arg(value_parser = ["set-dev", "unset-dev"])]
        action: Option<String>,

        /// ADB device serial
        #[arg(short = 's', long)]
        serial: Option<String>,
    },
    /// Open an installed AIX Agent Page on Rokid Glasses
    LaunchPage {
        /// Project directory or .aix file used to resolve Agent identity and Page routes
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Page path (defaults to the first declared Page)
        #[arg(value_name = "PATH")]
        path: Option<String>,

        /// Open as a card instead of full screen
        #[arg(long)]
        card: bool,

        /// Parameters as a JSON object
        #[arg(long, conflicts_with = "params_file")]
        params: Option<String>,

        /// Read parameters from a JSON file
        #[arg(long, value_name = "FILE")]
        params_file: Option<PathBuf>,

        /// ADB device serial
        #[arg(short = 's', long)]
        serial: Option<String>,
    },
        /// Configure and open an installed overlay Widget on Rokid Glasses
    LaunchWidget {
        /// Project directory or .aix file used to resolve Agent identity and Widget family
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Widget path declared in app.json
        #[arg(value_name = "PATH")]
        path: String,

        /// Widget grid start index; reuses its current or first free position by default
        #[arg(short, long)]
        position: Option<usize>,

        /// Parameters as a JSON object
        #[arg(long, conflicts_with = "params_file")]
        params: Option<String>,

        /// Read parameters from a JSON file
        #[arg(long, value_name = "FILE")]
        params_file: Option<PathBuf>,

        /// ADB device serial
        #[arg(short = 's', long)]
        serial: Option<String>,
    },
    /// Show or clear the current device Widget layout
    WidgetLayout {
        /// Optional explicit read-only action
        #[arg(value_parser = ["show"], conflicts_with = "clear")]
        action: Option<String>,

        /// Clear persistent and overlay Widget placements
        #[arg(long)]
        clear: bool,

        /// Skip confirmation for --clear
        #[arg(long, requires = "clear")]
        yes: bool,

        /// ADB device serial
        #[arg(short = 's', long)]
        serial: Option<String>,
    },
    /// Show the effective Agent Definition JSON
    Show {
        /// Project directory or .aix file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Definition JSON override
        #[arg(long, value_name = "FILE")]
        definition: Option<PathBuf>,

        /// Write JSON to a file instead of stdout
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Print compact single-line JSON
        #[arg(long)]
        compact: bool,
    },
    /// Install an AIX Agent on Rokid Glasses over ADB
    Install {
        /// Project directory or .aix file to install
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Definition JSON (defaults to <project>/agent.json)
        #[arg(long, value_name = "FILE")]
        definition: Option<PathBuf>,

        /// ADB device serial
        #[arg(short = 's', long)]
        serial: Option<String>,

        /// Enable optimization
        #[arg(short = 'O', long, default_value_t = false)]
        optimize: bool,

        /// Optimization level (1-3)
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
        opt_level: u8,

        /// Supported AIX engine version range
        #[arg(long)]
        engine: Option<String>,
    },
    /// Pack a directory into a .aix file
    Pack {
        /// Input directory to pack
        #[arg(value_name = "INPUT_DIR")]
        input_dir: PathBuf,

        /// Output .aix file path (optional, defaults to bundle.aix)
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: Option<PathBuf>,

        /// Enable optimization
        #[arg(short = 'O', long, default_value_t = false)]
        optimize: bool,

        /// Optimization level (1-3)
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
        opt_level: u8,

        /// Supported AIX engine version range
        #[arg(long)]
        engine: Option<String>,
    },
    /// List the contents of a .aix file
    #[command(alias = "ls")]
    List {
        /// Path to the .aix file
        #[arg(value_name = "AIX_FILE")]
        file: PathBuf,
    },
    /// Optimize an existing .aix file
    Optimize {
        /// Path to the input .aix file
        #[arg(value_name = "AIX_FILE")]
        file: PathBuf,

        /// Output .aix file path
        #[arg(short, long, value_name = "OUTPUT_FILE")]
        output: PathBuf,

        /// Optimization level (1-3)
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
        level: u8,
    },
}

fn main() {
    if let Err(error) = run() {
        print_error(&error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Device { action, serial } => match action.as_deref() {
            Some("set-dev") => launch::set_developer_mode(serial.as_deref(), true)?,
            Some("unset-dev") => launch::set_developer_mode(serial.as_deref(), false)?,
            None => launch::device_status(serial.as_deref())?,
            Some(_) => unreachable!(),
        },
        Commands::LaunchPage {
            input,
            path,
            card,
            params,
            params_file,
            serial,
        } => launch::launch_page(
            input,
            launch::PageLaunchOptions {
                path: path.as_deref(),
                card: *card,
                params: params.as_deref(),
                params_file: params_file.as_deref(),
                serial: serial.as_deref(),
            },
        )?,
        Commands::LaunchWidget {
            input,
            path,
            position,
            params,
            params_file,
            serial,
        } => launch::launch_widget(
            input,
            launch::WidgetLaunchOptions {
                path,
                position: *position,
                params: params.as_deref(),
                params_file: params_file.as_deref(),
                serial: serial.as_deref(),
            },
        )?,
        Commands::WidgetLayout {
            action: _,
            clear,
            yes,
            serial,
        } => {
            if *clear {
                confirm_clear(*yes)?;
                launch::clear_widget_layout(serial.as_deref())?;
            } else {
                launch::show_widget_layout(serial.as_deref())?;
            }
        }
        Commands::Show {
            input,
            definition,
            output,
            compact,
        } => show_definition(input, definition.as_deref(), output.as_deref(), *compact)?,
        Commands::Install {
            input,
            definition,
            serial,
            optimize,
            opt_level,
            engine,
        } => launch::install_agent(
            input,
            launch::InstallOptions {
                definition: definition.as_deref(),
                serial: serial.as_deref(),
                optimize: *optimize,
                opt_level: *opt_level,
                engine: engine.as_deref(),
            },
        )?,
        Commands::Pack {
            input_dir,
            output,
            optimize,
            opt_level,
            engine,
        } => {
            let output_path = output
                .clone()
                .unwrap_or_else(|| PathBuf::from("bundle.aix"));
            pack_directory(
                input_dir,
                &output_path,
                *optimize,
                *opt_level,
                engine.as_deref(),
            )?;
        }
        Commands::List { file } => {
            list_aix(file)?;
        }
        Commands::Optimize {
            file,
            output,
            level,
        } => optimize_aix(file, output, *level)?,
    }

    Ok(())
}

fn confirm_clear(yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        bail!("--clear requires confirmation; rerun with --yes in a non-interactive shell");
    }
    eprint!("Clear all permanent and dynamic Widget placements? [y/N] ");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        bail!("Widget layout clear cancelled");
    }
    Ok(())
}

fn show_definition(
    input: &Path,
    definition: Option<&Path>,
    output: Option<&Path>,
    compact: bool,
) -> Result<()> {
    let value = launch::resolve_definition(input, definition)?.value;
    let mut json = if compact {
        serde_json::to_string(&value)?
    } else {
        serde_json::to_string_pretty(&value)?
    };
    json.push('\n');
    if let Some(output) = output {
        std::fs::write(output, json)?;
    } else {
        print!("{json}");
    }
    Ok(())
}

fn print_error(error: &anyhow::Error) {
    if std::io::stderr().is_terminal() {
        eprintln!("\x1b[1;31m✖\x1b[0m \x1b[31m{error:?}\x1b[0m");
    } else {
        eprintln!("✖ {error:?}");
    }
}

pub(crate) fn pack_directory(
    src_dir: &Path,
    dst_file: &Path,
    optimize: bool,
    opt_level: u8,
    engine: Option<&str>,
) -> Result<()> {
    let uuid = Uuid::new_v4().to_string();
    println!("Generated UUID: {}", uuid);

    let output = aix_pack::collector::pack_directory(
        src_dir,
        &CollectOptions::default(),
        PackOptions {
            build_id: uuid,
            engine: engine.map(str::to_string),
            optimize: optimize.then(|| OptimizeOptions {
                level: opt_level,
                ..OptimizeOptions::default()
            }),
            signing_key: None,
        },
    )?;
    for warning in &output.warnings {
        eprintln!("warning: {}", warning);
    }
    std::fs::write(dst_file, output.data)?;

    for line in render_pack_report_lines(&output.report, optimize) {
        println!("{}", line);
    }

    let final_package_size = std::fs::metadata(dst_file)?.len();
    println!(
        "Package created: {} ({})",
        dst_file.display(),
        format_size(final_package_size)
    );

    Ok(())
}

fn render_pack_report_lines(report: &aix_pack::OptimizeReport, optimize: bool) -> Vec<String> {
    let mut lines = Vec::new();

    for file in &report.files {
        if let Some(line) = render_pack_file_line(file) {
            lines.push(line);
        }
    }

    if let Some(summary_line) = render_pack_summary_line(report, optimize) {
        lines.push(summary_line);
    }

    lines
}

fn render_pack_file_line(file: &aix_pack::FileOptimizeReport) -> Option<String> {
    let mut lines = Vec::new();
    if file.converted_to_utf8 {
        lines.push(format!("Converted {} to UTF-8 for packaging", file.path));
    }
    if file.status == aix_pack::OptimizeStatus::Optimized {
        lines.push(format!(
            "Optimized {}: {} -> {} (saved {})",
            file.path,
            format_size(file.original_size),
            format_size(file.output_size),
            format_size(file.saved_bytes)
        ));
    } else if file.path != "VERSION" {
        lines.push(format!("Adding file: {}", file.path));
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn render_pack_summary_line(report: &aix_pack::OptimizeReport, optimize: bool) -> Option<String> {
    if optimize && report.original_size > 0 {
        let ratio = (report.saved_bytes as f64 / report.original_size as f64) * 100.0;
        Some(format!(
            "Optimization Summary: Total saved {} ({:.2}%)",
            format_size(report.saved_bytes),
            ratio
        ))
    } else {
        None
    }
}

fn optimize_aix(input: &Path, output: &Path, level: u8) -> Result<()> {
    let data = std::fs::read(input)?;
    let optimized = aix_pack::optimize_package(
        &data,
        &OptimizeOptions {
            level,
            ..OptimizeOptions::default()
        },
    )?;
    std::fs::write(output, optimized.data)?;
    println!(
        "Optimized {:?} to {:?} (saved {})",
        input,
        output,
        format_size(optimized.report.saved_bytes)
    );
    Ok(())
}

fn list_aix(file_path: &Path) -> Result<()> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let reader = aix::AixReader::new(buffer)?;

    println!("Contents of {:?}:", file_path);
    for entry in reader.list() {
        println!(
            "{}: {} (compressed: {})",
            entry.name,
            aix::format_size(entry.size),
            aix::format_size(entry.compressed_size)
        );
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    aix::format_size(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aix_pack::{FileOptimizeReport, OptimizeReport, OptimizeStatus};
    use tempfile::tempdir;

    #[test]
    fn pack_directory_minifies_scripts_without_optimize_flag() {
        let temp_dir = tempdir().unwrap();
        let input_dir = temp_dir.path().join("input");
        let output_path = temp_dir.path().join("bundle.aix");
        std::fs::create_dir_all(input_dir.join("scripts")).unwrap();
        std::fs::write(input_dir.join("app.json"), br#"{"pages":[]}"#).unwrap();
        std::fs::write(
            input_dir.join("scripts/app.js"),
            b"function demo(value) { return value + 1; }\n",
        )
        .unwrap();

        pack_directory(&input_dir, &output_path, false, 2, Some("*")).unwrap();

        let reader = aix::AixReader::new(std::fs::read(&output_path).unwrap()).unwrap();
        let js_output = String::from_utf8(reader.read_file("scripts/app.js").unwrap()).unwrap();
        assert!(js_output.starts_with("function demo("));
        assert!(js_output.contains("return "));
        assert!(!js_output.contains("value"));
    }

    #[test]
    fn render_pack_report_keeps_script_minification_visible_without_summary() {
        let report = OptimizeReport {
            files: vec![
                FileOptimizeReport {
                    path: "VERSION".into(),
                    status: OptimizeStatus::Skipped,
                    original_size: 36,
                    output_size: 36,
                    saved_bytes: 0,
                    converted_to_utf8: false,
                },
                FileOptimizeReport {
                    path: "scripts/app.js".into(),
                    status: OptimizeStatus::Optimized,
                    original_size: 40,
                    output_size: 28,
                    saved_bytes: 12,
                    converted_to_utf8: false,
                },
            ],
            original_size: 76,
            output_size: 64,
            saved_bytes: 12,
        };

        let lines = render_pack_report_lines(&report, false);

        assert_eq!(
            lines,
            vec!["Optimized scripts/app.js: 40 bytes -> 28 bytes (saved 12 bytes)".to_string()]
        );
    }

    #[test]
    fn render_pack_report_adds_summary_only_for_optimize_mode() {
        let report = OptimizeReport {
            files: vec![FileOptimizeReport {
                path: "images/cover.png".into(),
                status: OptimizeStatus::Optimized,
                original_size: 4096,
                output_size: 3072,
                saved_bytes: 1024,
                converted_to_utf8: false,
            }],
            original_size: 4096,
            output_size: 3072,
            saved_bytes: 1024,
        };

        let lines = render_pack_report_lines(&report, true);

        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0],
            "Optimized images/cover.png: 4.00 KB -> 3.00 KB (saved 1.00 KB)"
        );
        assert_eq!(
            lines[1],
            "Optimization Summary: Total saved 1.00 KB (25.00%)"
        );
    }
}
