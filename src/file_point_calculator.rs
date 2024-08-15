use hashbrown::HashSet;

use crate::{
    file_processor::{self, get_ascii_readable_characters_set},
    pattern::Pattern,
    utils,
};

/// The maximum number of points to be awarded for entropy matching.
pub const MAX_ENTROPY_POINTS: f64 = 15.0;
/// The amount by which the total file count will be scaled to create the confidence factor.
pub const CONFIDENCE_SCALE_FACTOR: f64 = 1.0 / 3.0;
/// The number of points to be awarded for a file extension match.
pub const FILE_EXTENSION_POINTS: f64 = 5.0;

#[derive(Default)]
pub struct FilePointCalculator {}

impl FilePointCalculator {
    pub fn compute(pattern: &Pattern, chunk: &[u8], path: &str) -> usize {
        let mut frequencies = [0; 256];

        if pattern.data.scan_sequences || pattern.data.scan_composition {
            file_processor::count_byte_frequencies(chunk, &mut frequencies);
        }

        let mut points = 0.0;

        if pattern.data.scan_sequences {
            let (p, success) = Self::test_byte_sequences(pattern, chunk);

            // Byte sequence matches, if specified, MUST be present for a file to match the pattern.
            if !success {
                return 0;
            }

            points += p;
        }

        if pattern.data.scan_strings {
            points += Self::test_file_strings(pattern, chunk);
        }

        if pattern.data.scan_composition {
            points += Self::test_entropy_deviation(pattern, &frequencies);
        }

        // Scale the relevant points by the confidence factor derived from the total files scanned.
        points *= pattern.confidence_factor;

        // The file extension is considered a separate factor and doesn't scale with the number
        // of scanned files.
        points += Self::test_file_extension(pattern, path);

        points.round() as usize
    }

    #[inline(always)]
    pub fn test_byte_sequences(pattern: &Pattern, bytes: &[u8]) -> (f64, bool) {
        if !pattern.data.scan_sequences || pattern.data.sequences.is_empty() {
            return (0.0, true);
        }

        // By default, sequences are sorted by their starting index - largest first.
        // This means that the one with the largest position will be first.
        // In the best case, it might be outside the bounds of the file, thereby
        // letting is bail the loop early. Though this is likely something that will
        // only come up with small files.
        let bytes_len = bytes.len();
        let mut points = 0;
        for (start, sequence) in &pattern.data.sequences {
            let len = sequence.len();
            let end = start.saturating_add(len);
            if *start > bytes_len || end > bytes_len {
                return (0.0, false);
            }

            unsafe {
                if sequence != bytes.get_unchecked(*start..end) {
                    return (0.0, false);
                }
            }

            points += len;
        }

        (points as f64, true)
    }

    #[inline(always)]
    pub fn test_entropy_deviation(pattern: &Pattern, frequencies: &[usize; 256]) -> f64 {
        let reference_entropy = pattern.data.average_entropy;
        if !pattern.data.scan_composition || reference_entropy == 0.0 {
            return MAX_ENTROPY_POINTS;
        }

        // Compute the entropy for the target data block.
        let target_entropy = utils::calculate_shannon_entropy(frequencies);
        let absolute_diff = (reference_entropy - target_entropy).abs();
        let percentage_diff = if reference_entropy > 0.0 {
            (absolute_diff / reference_entropy) * 100.0
        } else {
            0.0
        };

        // Scale the points linearly between 0 and MAX_ENTROPY_POINTS based on the differences.
        MAX_ENTROPY_POINTS * (1.0 - percentage_diff / 100.0)
    }

    #[inline(always)]
    pub fn test_file_extension(pattern: &Pattern, path: &str) -> f64 {
        let ext = utils::get_file_extension(path);

        if pattern.type_data.known_extensions.contains(&ext) {
            FILE_EXTENSION_POINTS
        } else {
            0.0
        }
    }

    #[inline(always)]
    pub fn test_file_strings(pattern: &Pattern, bytes: &[u8]) -> f64 {
        if !pattern.data.scan_strings || pattern.data.strings.is_empty() {
            return 0.0;
        }

        let readable = get_ascii_readable_characters_set().clone();
        let strings: HashSet<String> =
            HashSet::from_iter(file_processor::extract_file_strings(bytes, &readable));

        pattern
            .data
            .strings
            .intersection(&strings)
            .map(|s| s.len() as f64)
            .sum()
    }
}
