use std::collections::HashMap;

use crate::{file_processor, pattern::Pattern, utils};

/// The maximum number of points to be awarded for entropy matching.
pub const MAX_ENTROPY_POINTS: f64 = 20.0;
/// The amount by which the total file count will be scaled to create the confidence factor.
pub const CONFIDENCE_SCALE_FACTOR: f64 = 1.0 / 3.0;
/// The number of points to be awarded for a file extension match.
pub const FILE_EXTENSION_POINTS: f64 = 5.0;

#[derive(Default)]
pub struct FilePointCalculator {}

impl FilePointCalculator {
    pub fn compute(pattern: &Pattern, path: &str) -> usize {
        let chunk = file_processor::read_file_header_chunk(path).expect("failed to read file");

        let mut frequencies = HashMap::new();
        file_processor::count_byte_frequencies(&chunk, &mut frequencies);

        let mut points = 0.0;

        if pattern.data.scan_byte_sequences {
            points += Self::test_byte_sequence(pattern, &chunk);

            // Byte sequence matches, if specified, MUST exist for a match to be valid at all.
            // If the points returned are zero, the file cannot be a match.
            // These should be tested before the strings and entropy.
            if points == 0.0 {
                return 0;
            }
        }

        if pattern.data.scan_strings {
            points += Self::test_file_strings(pattern, &chunk);
        }

        if pattern.data.scan_byte_distribution {
            points += Self::test_entropy_deviation(pattern, &frequencies);
        }

        // Scale the relevant points by the confidence factor derived from the total files scanned.
        points *= Self::get_confidence_factor(pattern);

        // The file extension is considered a separate factor and doesn't scale with the number
        // of scanned files.
        points += Self::test_file_extension(pattern, path);

        points.round() as usize
    }

    /// Computer the maximum number of points that can be awarded for a perfect match against this pattern.
    /// The more detailed the pattern, the higher the total points available.
    pub fn compute_max_points(pattern: &Pattern) -> usize {
        let mut points = 0.0;

        if pattern.data.scan_byte_sequences {
            for (_, sequence) in &pattern.data.byte_sequences {
                points += sequence.len() as f64;
            }
        }

        if pattern.data.scan_strings {
            for string in &pattern.data.string_patterns {
                points += string.len() as f64;
            }
        }

        if pattern.data.scan_byte_distribution {
            points += MAX_ENTROPY_POINTS;
        }

        // Scale the relevant points by the confidence factor derived from the total files scanned.
        points *= Self::get_confidence_factor(pattern);

        // The file extension is considered a separate factor and doesn't scale with the number
        // of scanned files.
        points += FILE_EXTENSION_POINTS;

        points.ceil() as usize
    }

    pub fn get_confidence_factor(pattern: &Pattern) -> f64 {
        (pattern.other_data.total_scanned_files as f64).powf(CONFIDENCE_SCALE_FACTOR)
    }

    pub fn test_byte_sequence(pattern: &Pattern, bytes: &[u8]) -> f64 {
        if !pattern.data.scan_byte_sequences || pattern.data.byte_sequences.is_empty() {
            return 0.0;
        }

        let mut points = 0.0;
        for (start, sequence) in &pattern.data.byte_sequences {
            let end = *start + sequence.len();
            if *start > bytes.len() || end > bytes.len() {
                points = 0.0;
                break;
            }

            if sequence != &bytes[*start..end] {
                points = 0.0;
                break;
            } else {
                points += sequence.len() as f64;
            }
        }

        points
    }

    pub fn test_entropy_deviation(pattern: &Pattern, frequencies: &HashMap<u8, usize>) -> f64 {
        let reference_entropy = pattern.data.get_entropy();
        if !pattern.data.scan_byte_distribution || reference_entropy == 0.0 {
            return MAX_ENTROPY_POINTS;
        }

        // Compute the entropy for the target block.
        let target_entropy = utils::calculate_shannon_entropy(frequencies);

        // Calculate the absolute percentage deviation.
        let absolute_diff = (reference_entropy - target_entropy).abs();
        let average_value = (reference_entropy + target_entropy) / 2.0;
        let percentage_diff = if average_value != 0.0 {
            (absolute_diff / average_value) * 100.0
        } else {
            0.0
        };

        // Scale the points linearly between 0 and MAX_ENTROPY_POINTS based on the differences.
        MAX_ENTROPY_POINTS * (1.0 - percentage_diff)
    }

    pub fn test_file_extension(pattern: &Pattern, path: &str) -> f64 {
        let ext = utils::get_file_extension(path);

        if pattern.type_data.known_extensions.contains(&ext) {
            FILE_EXTENSION_POINTS
        } else {
            0.0
        }
    }

    pub fn test_file_strings(pattern: &Pattern, bytes: &[u8]) -> f64 {
        if !pattern.data.scan_strings || pattern.data.string_patterns.is_empty() {
            return 0.0;
        }

        let strings = file_processor::generate_file_string_hashset(bytes);

        let mut points = 0.0;
        for str in &pattern.data.string_patterns {
            if strings.contains(str) {
                points += str.len() as f64;
            }
        }

        points
    }
}
