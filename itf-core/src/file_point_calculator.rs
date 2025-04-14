use hashbrown::HashSet;

use crate::{file_processor, pattern::Pattern, utils};

/// The maximum number of points to be awarded for entropy matching.
pub const ENTROPY_POINTS: usize = 15;
/// The number of points to be awarded for a file extension match.
pub const FILE_EXTENSION_POINTS: usize = 5;
/// The bonus points awarded per regular expression match.
pub const REGULAR_EXPRESSION_EXTRA_POINTS: usize = 15;

#[derive(Default)]
pub struct FilePointCalculator {}

impl FilePointCalculator {
    pub fn compute(pattern: &Pattern, chunk: &[u8], path: &str) -> usize {
        let mut frequencies = [0; 256];

        if pattern.data.should_scan_sequences() || pattern.data.should_scan_composition() {
            file_processor::count_byte_frequencies(chunk, &mut frequencies);
        }

        let mut points = 0;

        if pattern.data.should_scan_sequences() {
            let (p, success) = Self::test_byte_sequences(pattern, chunk);

            // Byte sequence matches, if specified, MUST be present for a file to match the pattern.
            if !success {
                return 0;
            }

            points += p;
        }

        if pattern.data.should_scan_regular_expressions() {
            points += Self::test_regular_expressions(pattern, chunk);
        }

        if pattern.data.should_scan_strings() {
            points += Self::test_file_strings(pattern, chunk);
        }

        if pattern.data.should_scan_composition() {
            points += Self::test_entropy(pattern, &frequencies);
        }

        // The file extension is considered a separate factor and doesn't scale with the number
        // of scanned files.
        points += Self::test_file_extension(pattern, path);

        points
    }

    #[inline(always)]
    fn test_byte_sequences(pattern: &Pattern, bytes: &[u8]) -> (usize, bool) {
        // By default, sequences are sorted by their starting index - largest first.
        // This means that the one with the largest position will be first.
        // In the best case, it might be outside the bounds of the file, thereby
        // letting us bail the loop early. Though this is likely something that will
        // only come up with small files.
        let bytes_len = bytes.len();
        let mut points = 0;
        for (start, sequence) in &pattern.data.sequences {
            let len = sequence.len();
            let end = start.saturating_add(len);
            if *start > bytes_len || end > bytes_len {
                return (0, false);
            }

            unsafe {
                if sequence != bytes.get_unchecked(*start..end) {
                    return (0, false);
                }
            }

            points += len;
        }

        (points, true)
    }

    #[inline(always)]
    fn test_regular_expressions(pattern: &Pattern, bytes: &[u8]) -> usize {
        let mut points = 0;
        for re in &pattern.data.regexes {
            if re.find(bytes).is_some() {
                points += re.as_str().len() + REGULAR_EXPRESSION_EXTRA_POINTS;
            }
        }

        points
    }

    #[inline(always)]
    fn test_entropy(pattern: &Pattern, frequencies: &[usize; 256]) -> usize {
        let reference_min_entropy = pattern.data.min_entropy;
        let reference_max_entropy = pattern.data.max_entropy;
        if !pattern.data.should_scan_composition()
            || reference_min_entropy == 0
            || reference_max_entropy == 0
        {
            return 0;
        }

        // Compute the entropy for the target data block.
        let target_entropy = utils::calculate_shannon_entropy_fixed(frequencies);
        if target_entropy < reference_min_entropy || target_entropy > reference_max_entropy {
            return 0;
        }

        ENTROPY_POINTS
    }

    #[inline(always)]
    fn test_file_extension(pattern: &Pattern, path: &str) -> usize {
        if pattern
            .type_data
            .known_extensions
            .contains(&utils::get_file_extension(path))
        {
            FILE_EXTENSION_POINTS
        } else {
            0
        }
    }

    #[inline(always)]
    fn test_file_strings(pattern: &Pattern, bytes: &[u8]) -> usize {
        let strings: HashSet<String> =
            HashSet::from_iter(file_processor::extract_file_strings(bytes));

        pattern
            .data
            .strings
            .intersection(&strings)
            .map(|s| s.len())
            .sum()
    }
}
