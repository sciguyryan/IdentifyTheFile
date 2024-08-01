pub mod file_processor;
pub mod pattern_file;
pub mod utils;

use std::collections::HashSet;

use pattern_file::Pattern;

const VERBOSE: bool = false;

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
    println!("Average Entropy = {}", pattern.data.average_entropy);
    println!("{half_splitter}");

    println!("{splitter}");
    println!("Matching positional byte sequences");
    file_processor::print_byte_sequence_matches(&pattern.data.byte_sequences);

    if pattern.data.string_patterns.is_empty() {
        println!("No common strings were found!");
    }

    println!("{splitter}");
    println!("Common strings = {:?}", pattern.data.string_patterns);
    println!("{splitter}");

    let ref_chars: HashSet<u8> = file_processor::STRING_CHARS.iter().copied().collect();

    println!("Testing common string matches...");
    if file_processor::test_matching_file_strings(
        file_dir,
        target_extension,
        &ref_chars,
        &pattern.data.string_patterns,
    ) {
        println!("\x1b[92mSuccessfully matched all applicable strings!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more strings!\x1b[0m");
    }

    println!("Testing common byte sequence matches...");
    if file_processor::test_matching_file_byte_sequences(
        file_dir,
        target_extension,
        &pattern.data.byte_sequences,
    ) {
        println!("\x1b[92mSuccessfully matched all applicable byte sequences!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more byte sequences!\x1b[0m");
    }
}
