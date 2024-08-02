use std::collections::HashMap;

use crate::{file_processor, pattern_file::Pattern, utils};

const MAX_ENTROPY_POINTS: f64 = 50.0;
const MAX_DEVIATION_PERCENT: f64 = 100.0;
const MIN_DEVIATION_PERCENT: f64 = 1.0;

#[derive(Default)]
pub struct FilePointCalculator {
    pub points: usize,
}

impl FilePointCalculator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn test_byte_sequence(&mut self, bytes: &[u8], pattern: &Pattern) -> bool {
        if !pattern.data.scan_byte_sequences || pattern.data.byte_sequences.is_empty() {
            return true;
        }

        let mut points = 0;
        let mut is_match = true;
        for (start, sequence) in &pattern.data.byte_sequences {
            let end = *start + sequence.len();
            if *start > bytes.len() || end > bytes.len() {
                is_match = false;
                break;
            }

            if sequence != &bytes[*start..end] {
                is_match = false;
                break;
            } else {
                points += sequence.len();
            }
        }

        if is_match {
            self.points += points;
        }

        is_match
    }

    pub fn test_file_strings(&mut self, bytes: &[u8], pattern: &Pattern) -> bool {
        if !pattern.data.scan_strings || pattern.data.string_patterns.is_empty() {
            return true;
        }

        let strings = file_processor::generate_file_string_hashset(bytes);

        let mut points = 0;
        let mut matches = 0;
        for str in &pattern.data.string_patterns {
            if strings.contains(str) {
                points += str.len();
                matches += 1;
            }
        }

        self.points += points;

        matches == pattern.data.string_patterns.len()
    }

    pub fn test_entropy_deviation(&mut self, bytes: &[u8], pattern: &Pattern) -> f64 {
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
            // If deviation is less than the minimum, award the maximum points.
            self.points += MAX_ENTROPY_POINTS as usize;

            return MAX_ENTROPY_POINTS;
        }

        // Calculate the score linearly between 0 and 150.
        let score = MAX_ENTROPY_POINTS
            * (1.0
                - (deviation_percentage - MIN_DEVIATION_PERCENT)
                    / (MAX_DEVIATION_PERCENT - MIN_DEVIATION_PERCENT));
        self.points += score as usize;

        MAX_ENTROPY_POINTS
    }
}
