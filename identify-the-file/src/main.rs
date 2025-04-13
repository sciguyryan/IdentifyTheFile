#![crate_name = "identify_the_file"]

use clap::{Parser, Subcommand};
use itf_core::{
    file_point_calculator::FilePointCalculator, file_processor, pattern::Pattern,
    pattern_handler::PatternHandler, utils,
};
use prettytable::{Cell, Row, Table};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{env, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "Identify The File",
    about = "A CLI application designed to identify files or build patterns to aid with file type identification.",
    version = "0.1.0",
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
        pattern_source_dir: String,

        #[arg(short, long, default_value = "", value_name = "example.mkv.json")]
        target_pattern: String,

        #[arg(short, long, default_value_t = -1)]
        result_count: i32,

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
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Identify {
            pattern_source_dir: _,
            target_pattern: _,
            result_count: _,
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
    }
}

fn built_pattern_handler(source_directory: &str, target_pattern: &str) -> PatternHandler {
    let mut pattern_handler = PatternHandler::default();

    // By default we'll look at the path /patterns/ relative to the path of the executable.
    // If the source path is specified then we will attempt to load the patterns from there instead.
    let pattern_source = if source_directory.is_empty() {
        if let Ok(p) = env::current_dir() {
            let mut temp = p.clone();
            temp.push("patterns");
            temp
        } else {
            eprintln!("Unable to get the current working directory, and no definition source specified. Unable to continue.");
            return pattern_handler;
        }
    } else {
        PathBuf::from(source_directory)
    };

    if !utils::directory_exists(&pattern_source) {
        eprintln!("The specified pattern source directory doesn't exist. Unable to continue.");
        return pattern_handler;
    }

    pattern_handler.read(pattern_source, target_pattern);

    pattern_handler
}

#[inline]
fn match_patterns<'a>(pattern_handler: &'a PatternHandler, path: &str) -> Vec<PatternMatch<'a>> {
    let chunk = file_processor::read_file_header_chunk(path).expect("failed to read sample file");

    let mut point_store: Vec<PatternMatch> = pattern_handler
        .patterns
        .par_iter()
        .filter_map(|pattern| {
            let points = FilePointCalculator::compute(pattern, &chunk, path, true);
            if points > 0 {
                Some(PatternMatch::new(
                    &pattern.type_data.uuid,
                    points,
                    pattern.max_points,
                ))
            } else {
                None
            }
        })
        .collect();

    // Sort the results by percentage match score, descending.
    point_store.sort_unstable_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

    point_store
}

#[derive(Debug)]
struct PatternMatch<'a> {
    pub uuid: &'a str,
    pub points: usize,
    pub max_points: usize,
    pub percentage: f32,
}

impl<'a> PatternMatch<'a> {
    pub fn new(uuid: &'a str, points: usize, max_points: usize) -> Self {
        Self {
            uuid,
            points,
            max_points,
            percentage: utils::round_to_dp(points as f32 / max_points as f32 * 100.0, 1),
        }
    }
}

fn print_results(results: &[PatternMatch], handler: &PatternHandler) {
    let mut table = Table::new();

    // Add a row for the header.
    table.add_row(Row::new(vec![
        Cell::new("Rank").style_spec("b"),
        Cell::new("Name").style_spec("b"),
        Cell::new("Points").style_spec("b"),
        Cell::new("Max Points").style_spec("b"),
        Cell::new("Percentage").style_spec("b"),
    ]));

    for (i, result) in results.iter().enumerate() {
        let p = handler
            .patterns
            .iter()
            .find(|pattern| pattern.type_data.uuid == result.uuid)
            .unwrap();

        // The values are rounded to 1 d.p., so we don't need to worry about the edge-case
        // floating point issues.
        let colour = match result.percentage {
            0.0..=33.3 => "Fr",
            33.4..=66.66 => "Fy",
            66.67..=100.0 => "Fg",
            _ => "Fw",
        };

        table.add_row(Row::new(vec![
            Cell::new(&(i + 1).to_string()).style_spec(colour),
            Cell::new(&p.type_data.name).style_spec(colour),
            Cell::new(&result.points.to_string()).style_spec(colour),
            Cell::new(&result.max_points.to_string()).style_spec(colour),
            Cell::new(&result.percentage.to_string()).style_spec(colour),
        ]));
    }

    table.printstd();
}

fn process_identify_command(cmd: &Commands) {
    if let Commands::Identify {
        pattern_source_dir: source_directory,
        target_pattern,
        result_count,
        file,
    } = cmd
    {
        if !utils::file_exists(file) {
            eprintln!("The specified sample file path '{file}' doesn't exist.");
            return;
        }

        let pattern_handler = built_pattern_handler(source_directory, target_pattern);
        if pattern_handler.is_empty() {
            eprintln!("No applicable patterns were found. Unable to continue.");
            return;
        }

        let mut results = match_patterns(&pattern_handler, file);

        // Only retail a set number of results, if specified.
        if *result_count != -1 {
            results.truncate(*result_count as usize);
        }

        print_results(&results, &pattern_handler);
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

        //let now = std::time::Instant::now();

        pattern.build_patterns_from_data(
            path,
            extension,
            !*no_strings,
            !*no_sequences,
            !*no_composition,
        );

        //println!("{}", now.elapsed().as_secs_f64());

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
