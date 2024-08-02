use std::collections::HashMap;

use crate::{file_processor, pattern::Pattern, utils};

/// The maximum number of points to be awarded for entropy matching.
pub const MAX_ENTROPY_POINTS: f64 = 25.0;
/// The minimum percentage deviation to be used in the entropy points calculation.
const MIN_DEVIATION_PERCENT: f64 = 1.0;
/// The maximum percentage deviation to be used in the entropy points calculation.
const MAX_DEVIATION_PERCENT: f64 = 100.0;
/// The amount by which the total file count will be scaled to create the confidence factor.
pub const CONFIDENCE_SCALE_FACTOR: f64 = 1.0 / 3.0;
/// The number of points to be awarded for a file extension match.
pub const FILE_EXTENSION_POINTS: f64 = 5.0;

#[derive(Default)]
pub struct FilePointCalculator {}

impl FilePointCalculator {
    pub fn compute(pattern: &Pattern, path: &str) -> usize {
        let chunk = file_processor::read_file_header_chunk(path).expect("failed to read file");

        let mut points = 0.0;

        if pattern.data.scan_byte_sequences {
            points += FilePointCalculator::test_byte_sequence(pattern, &chunk);

            // Byte sequence matches, if specified, MUST exist for a match to be valid at all.
            // If the points returned are zero, the file cannot be a match.
            // These should be tested before the strings and entropy.
            if points == 0.0 {
                return 0;
            }
        }

        if pattern.data.scan_strings {
            points += FilePointCalculator::test_file_strings(pattern, &chunk);
        }

        if pattern.data.scan_entropy {
            points += FilePointCalculator::test_entropy_deviation(pattern, &chunk);
        }

        points += FilePointCalculator::test_file_extension(pattern, path);

        let confidence_factor = FilePointCalculator::get_confidence_factor(pattern);

        (points * confidence_factor).round() as usize
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
                break;
            }

            if sequence != &bytes[*start..end] {
                break;
            } else {
                points += sequence.len() as f64;
            }
        }

        points
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

    pub fn test_entropy_deviation(pattern: &Pattern, bytes: &[u8]) -> f64 {
        let reference_entropy = pattern.data.get_entropy();
        if !pattern.data.scan_entropy || reference_entropy == 0.0 {
            return MAX_ENTROPY_POINTS;
        }

        // Compute the entropy for the target block.
        let mut frequencies = HashMap::new();
        file_processor::count_byte_frequencies(bytes, &mut frequencies);
        let target_entropy = file_processor::calculate_shannon_entropy(&frequencies);

        // Round the target and reference entropy to 3 decimal places.
        // Due to the complexities of working with floats, this ensures our results should be
        // relatively consistent.
        let rounded_reference_entropy = utils::round_to_dp(reference_entropy, 3);
        let rounded_target_entropy = utils::round_to_dp(target_entropy, 3);

        // Calculate the absolute percentage deviation.
        let deviation_percentage = ((rounded_target_entropy - rounded_reference_entropy).abs()
            / rounded_reference_entropy)
            * 100.0;
        if deviation_percentage >= MAX_DEVIATION_PERCENT {
            // If deviation is 100% or more, award a score of 0.
            return 0.0;
        }

        if deviation_percentage <= MIN_DEVIATION_PERCENT {
            return MAX_ENTROPY_POINTS;
        }

        // Calculate the score linearly between 0 and MAX_ENTROPY_POINTS.
        MAX_ENTROPY_POINTS
            * (1.0
                - (deviation_percentage - MIN_DEVIATION_PERCENT)
                    / (MAX_DEVIATION_PERCENT - MIN_DEVIATION_PERCENT))
    }

    pub fn test_file_extension(pattern: &Pattern, path: &str) -> f64 {
        let ext = utils::get_file_extension(path);

        if pattern.type_data.known_extensions.contains(&ext) {
            FILE_EXTENSION_POINTS
        } else {
            0.0
        }
    }
}
