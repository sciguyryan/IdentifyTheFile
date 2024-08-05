#[derive(Debug)]
pub struct ArgParser {
    pub user_name: String,
    pub user_email: String,
    pub identify_file_path: String,
    pub pattern_target_folder: String,
    pub pattern_target_extension: String,
    pub pattern_target_recursive: bool,
    pub help: bool,
}

impl From<&[String]> for ArgParser {
    fn from(value: &[String]) -> Self {
        let mut user_name = "";
        let mut user_email = "";
        let mut identify_file_path = "";
        let mut pattern_target_folder = "";
        let mut pattern_target_extension = "";
        let mut pattern_target_recursive = true;
        let mut help = false;

        for (i, arg) in value.iter().enumerate() {
            let next_index = i + 1;
            let lower_arg = arg.to_lowercase();
            if (lower_arg == "--user" || lower_arg == "-u") && next_index < value.len() {
                user_name = &value[i + 1];
            }

            if (lower_arg == "--email" || lower_arg == "-e") && next_index < value.len() {
                user_email = &value[i + 1];
            }

            if (lower_arg == "--identify" || lower_arg == "-i") && next_index < value.len() {
                identify_file_path = &value[i + 1];
            }

            if (lower_arg == "--pattern" || lower_arg == "-p") && next_index < value.len() {
                pattern_target_folder = &value[i + 1];
            }

            if (lower_arg == "--extension" || lower_arg == "-e") && next_index < value.len() {
                pattern_target_extension = &value[i + 1];
            }

            if lower_arg == "--no-recursive" || lower_arg == "-nr" {
                pattern_target_recursive = true;
            }

            if lower_arg == "--help" || lower_arg == "-h" {
                help = true;
            }
        }

        Self {
            user_name: user_name.to_string(),
            user_email: user_email.to_string(),
            identify_file_path: identify_file_path.to_string(),
            pattern_target_folder: pattern_target_folder.to_string(),
            pattern_target_extension: pattern_target_extension.to_string(),
            pattern_target_recursive,
            help,
        }
    }
}
