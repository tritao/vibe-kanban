mod completion;
mod flags;
mod registry;
mod types;

pub(crate) use completion::{
    apply_composer_autocomplete, composer_completion_items, composer_is_slash_mode,
    move_composer_autocomplete,
};
pub(crate) use flags::{parse_flags, parse_flags_mixed};
pub(crate) use registry::{
    canonical_command_name, flags_for_command, flags_for_subcommand, help_section_lines,
    help_syntax_for_command, help_syntax_for_subcommand, unknown_command_error,
    unknown_subcommand_error, usage_for_command,
};
