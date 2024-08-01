pub mod file_processor;
pub mod pattern_file;

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

const VERBOSE: bool = false;

fn main() {
    let splitter = "-".repeat(54);
    let half_splitter = "-".repeat(27);

    let file_dir = "D:\\GitHub\\IdentifyTheFile\\samples\\mkv";
    //let file_dir = "D:\\Downloads\\YouTube";
    let target_extension = "mkv";
    let files = file_processor::list_files_of_type(file_dir, target_extension);

    let ref_chars: HashSet<u8> = file_processor::STRING_CHARS.iter().copied().collect();

    let mut common_byte_sequences = HashMap::new();
    let mut first_byte_sequence_pass = true;

    let mut common_strings = Vec::new();

    let mut entropy = Vec::new();

    for file_path in &files {
        if VERBOSE {
            println!("Analyzing candidate file - {file_path}");
        }

        // If we made it here then we have a valid file.
        let chunk = file_processor::read_file_header_chunk(file_path).expect("failed to read file");

        entropy.push((file_path, file_processor::calculate_shannon_entropy(&chunk)));

        let new_hashset = file_processor::generate_file_string_hashset(&chunk, &ref_chars);
        common_strings.push(new_hashset);

        // On the first pass, we simply set the matching sequence as the entire byte block.
        // This will get trimmed down and split into sections over future loop iterations.
        if first_byte_sequence_pass {
            common_byte_sequences.insert(0, chunk);
            first_byte_sequence_pass = false;
            continue;
        }

        file_processor::refine_common_byte_sequences_v2(&chunk, &mut common_byte_sequences);
    }

    file_processor::strip_invalid_length_sequences(&mut common_byte_sequences);

    /*println!("{splitter}");
    let max_entropy = shannon_entropy
        .iter()
        .cloned()
        .fold(None, |max, x| match max {
            None => Some(x),
            Some(y) => Some(y.max(x)),
        })
        .unwrap();

    let min_entropy = shannon_entropy
        .iter()
        .cloned()
        .fold(None, |max, x| match max {
            None => Some(x),
            Some(y) => Some(y.min(x)),
        })
        .unwrap();*/

    let sum_entropy: f64 = entropy.iter().map(|(_, (b, _))| b).sum();
    let average_entropy = sum_entropy / (entropy.len() as f64);
    //let variation = ((max_entropy - min_entropy) / min_entropy) * 100f64;

    println!("{splitter}");
    println!("Valid sample files scanned: {}", files.len());
    println!("{splitter}");
    //println!("Maximum Entropy = {max_entropy}");
    //println!("Minimum Entropy = {min_entropy}");
    println!("Average Entropy = {average_entropy}");
    //println!("Entropy Variation = {variation}%");
    println!("{half_splitter}");
    println!("Entry deviations");
    /*let deviations: Vec<f64> = entropy
        .iter()
        .map(|(_, value)| ((value - average_entropy).abs() / average_entropy) * 100.0)
        .collect();
    println!("{deviations:?}");*/

    println!("{splitter}");
    println!("Matching positional byte sequences");
    file_processor::print_byte_sequence_matches(&common_byte_sequences);

    if common_strings.is_empty() {
        println!("No common strings were found!");
    }

    println!("{splitter}");
    println!("Starting string sieve (v2a)...");
    let before_v2a = Instant::now();
    let common_strings_hashset_v2a =
        file_processor::common_string_identification_v2a(&mut common_strings);
    println!("Elapsed time (v2a): {:.2?}", before_v2a.elapsed());
    println!("{}", common_strings_hashset_v2a.len());
    println!("{splitter}");
    let common_strings_hashset = common_strings_hashset_v2a;
    println!("Final common strings = {common_strings_hashset:?}");
    println!("{splitter}");

    println!("Testing common string matches...");
    if file_processor::test_matching_file_strings(
        file_dir,
        target_extension,
        &ref_chars,
        &common_strings_hashset,
    ) {
        println!("\x1b[92mSuccessfully matched all applicable strings!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more strings!\x1b[0m");
    }

    println!("Testing common byte sequence matches...");
    if file_processor::test_matching_file_byte_sequences(
        file_dir,
        target_extension,
        &common_byte_sequences,
    ) {
        println!("\x1b[92mSuccessfully matched all applicable byte sequences!\x1b[0m");
    } else {
        println!("\x1b[91mFailed to match one or more byte sequences!\x1b[0m");
    }
}
