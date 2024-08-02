pub mod file_point_calculator;
pub mod file_processor;
pub mod pattern_file;
pub mod utils;

use file_point_calculator::FilePointCalculator;
use pattern_file::Pattern;

fn main() {
    let splitter = "-".repeat(54);
    let half_splitter = "-".repeat(27);

    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples\\mkv";
    //let file_dir = "D:\\Downloads\\YouTube";
    let target_extension = "mkv";

    let mut pattern = Pattern::new("test", "test", vec![], vec![]);
    pattern.build_patterns_from_data(file_dir, target_extension, true, true, true);

    println!("{splitter}");
    println!(
        "Valid sample files scanned: {}",
        pattern.other_data.total_scanned_files
    );
    println!("{splitter}");
    println!("Average Entropy = {}", pattern.data.get_entropy());
    println!("{half_splitter}");

    println!("{splitter}");
    println!("Matching positional byte sequences");
    utils::print_byte_sequence_matches(&pattern.data.byte_sequences);

    if pattern.data.string_patterns.is_empty() {
        println!("No common strings were found!");
    }

    println!("{splitter}");
    println!("Common strings = {:?}", pattern.data.string_patterns);
    println!("{splitter}");

    // Test files here.
    let mut all_strings_match = true;
    let mut all_bytes_match = true;
    let files = utils::list_files_of_type(file_dir, target_extension);
    for file in &files {
        println!("File = {file}");
        let chunk = file_processor::read_file_header_chunk(file).expect("failed to read file");

        let mut calc = FilePointCalculator::new();
        all_bytes_match &= calc.test_byte_sequence(&chunk, &pattern);
        all_strings_match &= calc.test_file_strings(&chunk, &pattern);
        calc.test_entropy_deviation(&chunk, &pattern);

        println!("Total points = {}", calc.points);
    }

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
