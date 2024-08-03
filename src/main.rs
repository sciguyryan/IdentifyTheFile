pub mod file_point_calculator;
pub mod file_processor;
pub mod pattern;
pub mod utils;

use std::{env, time::Instant};

use file_point_calculator::FilePointCalculator;
use pattern::Pattern;

const VERBOSE: bool = false;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut user_name = "";
    let mut user_email = "";

    for (i, arg) in args.iter().enumerate() {
        let next_index = i + 1;
        if (arg == "--user" || arg == "-u") && next_index < args.len() {
            user_name = &args[i + 1];
        }

        if (arg == "--email" || arg == "-e") && next_index < args.len() {
            user_email = &args[i + 1];
        }
    }

    let splitter = "-".repeat(54);
    let half_splitter = "-".repeat(27);

    let sample_directory = "D:\\Storage\\File Type Samples\\mkv";
    let target_extension = "mkv";

    let processing_start = Instant::now();

    let mut pattern = Pattern::new("test waffles", "test", vec!["mkv".to_string()], vec![]);
    pattern.build_patterns_from_data(sample_directory, target_extension, true, true, true);
    pattern.add_submitter_data(user_name, user_email);
    let max_points = pattern.compute_max_points();

    println!("{:?}", pattern.write("D:\\temp\\"));

    println!(
        "Elapsed processing time: {:.2?}",
        processing_start.elapsed()
    );

    println!("{splitter}");
    let json = serde_json::to_string(&pattern).expect("");
    println!("{json}");
    println!("{splitter}");
    println!(
        "Valid sample files scanned: {}",
        pattern.other_data.total_scanned_files
    );
    println!("{splitter}");
    println!("Average Entropy = {}", pattern.data.get_entropy());
    println!("{splitter}");
    if pattern.data.byte_sequences.is_empty() {
        println!("No common byte sequences were found!");
    } else {
        println!("Matching positional byte sequences...");
        utils::print_byte_sequence_matches(&pattern.data.byte_sequences);
    }
    println!("{splitter}");
    if pattern.data.string_patterns.is_empty() {
        println!("No common strings were found!");
    } else {
        println!("Common strings...");
        println!("{:?}", pattern.data.string_patterns);
    }
    println!("{splitter}");

    let start_testing = Instant::now();

    // Test files here.
    let mut all_strings_match = true;
    let mut all_bytes_match = true;
    let files = utils::list_files_of_type("D:\\Downloads\\YouTube", target_extension);
    files.iter().for_each(|file| {
        //println!("File = {file}");

        let chunk = file_processor::read_file_header_chunk(file).expect("failed to read file");

        if pattern.data.scan_byte_sequences {
            let byte_points = FilePointCalculator::test_byte_sequence(&pattern, &chunk);
            if VERBOSE {
                println!("byte_points = {byte_points}");
            }
            all_bytes_match &= byte_points > 0.0 || pattern.data.byte_sequences.is_empty();
        }

        if pattern.data.scan_strings {
            let string_points = FilePointCalculator::test_file_strings(&pattern, &chunk);
            if VERBOSE {
                println!("string_points = {string_points}");
            }
            all_strings_match &= string_points > 0.0 || pattern.data.string_patterns.is_empty();
        }

        if pattern.data.scan_entropy {
            let entropy_points = FilePointCalculator::test_entropy_deviation(&pattern, &chunk);
            if VERBOSE {
                println!("entropy_points = {entropy_points}");
            }
        }

        let extension_points = FilePointCalculator::test_file_extension(&pattern, file);
        if VERBOSE {
            println!("extension_points = {extension_points}");
        }

        if VERBOSE {
            println!(
                "confidence_factor = {}",
                FilePointCalculator::get_confidence_factor(&pattern)
            );
        }

        if VERBOSE {
            let total_points = FilePointCalculator::compute(&pattern, file);
            println!("Total points = {total_points} of {max_points:?}");
            println!("{half_splitter}");
        }
    });

    println!("Elapsed testing time: {:.2?}", start_testing.elapsed());

    if all_bytes_match {
        println!("\x1b[92mSuccessfully matched all applicable byte sequences!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more byte sequences!\x1b[0m");
    }

    if all_strings_match {
        println!("\x1b[92mSuccessfully matched all applicable strings!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more strings!\x1b[0m");
    }
}
