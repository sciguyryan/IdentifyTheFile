mod file_point_calculator;
mod file_processor;
mod pattern;
mod pattern_handler;
mod utils;

use clap::{Parser, Subcommand};
use pattern_handler::PatternHandler;
use std::{env, path::PathBuf};

use pattern::Pattern;

#[derive(Parser)]
#[command(
    name = "Identify The File",
    about = "A CLI application designed to identify files or build patterns to aid with file type identification.",
    version = "1.0",
    author = "sciguyryan <sciguyryan@gmail.com>"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Identify {
        #[arg(short, long, default_value = "", value_name = "DIR")]
        source_directory: String,

        #[arg(short, long, default_value = "", value_name = "example.mkv.json")]
        target_pattern: String,

        #[arg(value_name = "FILE")]
        file: String,
    },
    Pattern {
        #[arg(short, long, default_value = "")]
        user_name: String,

        #[arg(short, long, default_value = "")]
        email: String,

        #[arg(short, long, default_value = "")]
        name: String,

        #[arg(short, long, default_value = "")]
        description: String,

        #[arg(short, long, default_value = "")]
        known_extensions: String,

        #[arg(short, long, default_value = "")]
        mimetypes: String,

        #[arg(long, default_value_t = false)]
        no_strings: bool,

        #[arg(long, default_value_t = false)]
        no_sequences: bool,

        #[arg(long, default_value_t = false)]
        no_composition: bool,

        #[arg(value_name = "EXT")]
        extension: String,

        #[arg(value_name = "PATH")]
        path: String,

        #[arg(value_name = "OUTPUT_DIR")]
        output_directory: Option<String>,
    },
    Refine {},
}

fn main() {
    /*let json =
        std::fs::read_to_string("D:\\Storage\\File Type Samples\\patterns\\matroska.json").unwrap();
    let pattern = pattern::from_simd_json_str(&json).unwrap();
    let fpc = FilePointCalculator::compute(
        &pattern,
        "D:\\Storage\\File Type Samples\\samples\\mkv\\The World in HDR.mkv",
    );
    println!("{}", fpc);
    return;*/

    let cli = Cli::parse();

    match &cli.command {
        Commands::Identify {
            source_directory: _,
            target_pattern: _,
            file: _,
        } => {
            process_identify_command(&cli.command);
        }
        Commands::Pattern {
            user_name: _,
            email: _,
            name: _,
            description: _,
            known_extensions: _,
            mimetypes: _,
            no_strings: _,
            no_sequences: _,
            no_composition: _,
            extension: _,
            path: _,
            output_directory: _,
        } => {
            process_pattern_command(&cli.command);
        }
        Commands::Refine {} => {
            todo!();
        }
    }
}

fn process_identify_command(cmd: &Commands) {
    if let Commands::Identify {
        source_directory,
        target_pattern,
        file,
    } = cmd
    {
        if !utils::file_exists(file) {
            eprintln!("The specified sample file path '{file}' doesn't exist.");
            return;
        }

        let mut pattern_handler = PatternHandler::default();
        let mut pattern_source = PathBuf::new();

        // By default we'll look at the path /patterns/ relative to the path of the executable.
        // If the source path is specified then we will attempt to load the patterns from there instead.
        if source_directory.is_empty() {
            if let Ok(p) = env::current_dir() {
                pattern_source = p.clone();
                pattern_source.push("patterns");
            } else {
                eprintln!("Unable to get the current working directory, and no definition source specified. Unable to continue.");
            }
        } else {
            pattern_source = PathBuf::from(source_directory);
        }

        if !utils::directory_exists(&pattern_source) {
            eprintln!("The specified pattern source directory doesn't exist. Unable to continue.");
            return;
        }

        pattern_handler.read(pattern_source, target_pattern);

        if pattern_handler.is_empty() {
            eprintln!("No applicable patterns were found. Unable to continue.");
            return;
        }

        // Add logic for identifying the file.
        println!("identify the file here...");
    }
}

fn process_pattern_command(cmd: &Commands) {
    if let Commands::Pattern {
        user_name,
        email,
        name,
        description,
        known_extensions,
        mimetypes,
        no_strings,
        no_sequences,
        no_composition,
        extension,
        path,
        output_directory,
    } = cmd
    {
        if !utils::directory_exists(path) {
            eprintln!("The specified target folder '{path}' doesn't exist.");
            return;
        }

        let extension = extension.trim_start_matches('.');
        if extension.is_empty() {
            eprintln!("The target extension may not be empty.");
            return;
        }

        if *no_strings && *no_sequences && *no_composition {
            eprintln!(
                "No pattern matching options were enabled, therefore no pattern can be created."
            );
            return;
        }

        let mut extensions: Vec<String> = if known_extensions.is_empty() {
            vec![]
        } else {
            known_extensions
                .split(',')
                .collect::<Vec<&str>>()
                .iter()
                .map(|s| s.to_uppercase())
                .collect()
        };

        let upper_ext = extension.to_uppercase();
        if !extensions.contains(&upper_ext) {
            extensions.push(upper_ext);
        }

        let mimetypes: Vec<String> = if mimetypes.is_empty() {
            vec![]
        } else {
            mimetypes
                .split(',')
                .collect::<Vec<&str>>()
                .iter()
                .map(|s| s.to_string())
                .collect()
        };

        let mut pattern = Pattern::new(name, description, extensions, mimetypes);
        pattern.add_submitter_data(user_name, email);
        pattern.build_patterns_from_data(
            path,
            extension,
            !*no_strings,
            !*no_sequences,
            !*no_composition,
        );

        if let Some(d) = output_directory {
            if !utils::directory_exists(d) {
                return;
            }

            if let Err(e) = pattern.write(d) {
                eprintln!("Failed to write pattern file: {e:?}");
            } else {
                println!(
                    "The pattern file has been successfully written to the specified directory!"
                );
            }
        } else {
            let json = serde_json::to_string(&pattern).unwrap();
            println!("{json}");
        }
    }
}
