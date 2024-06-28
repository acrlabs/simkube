use std::path::PathBuf;

use clap_complete::Shell;

use super::completions::{
    default_path_for,
    prompt_for_location,
};
use super::*;

#[rstest]
#[case::abs_path("/foo/bar", "/foo/bar")]
#[case::home_dir("~/foo/bar", dirs::home_dir().unwrap())]
#[case::no_entry("\n", default_path_for(&Shell::Bash))]
fn test_prompt_for_location(#[case] path: &str, #[case] expected_prefix: PathBuf) {
    let res = prompt_for_location(&Shell::Bash, &mut path.as_bytes()).unwrap();
    assert_starts_with!(res, &expected_prefix);
}

#[rstest]
fn test_prompt_for_location_unsupported() {
    let _ = prompt_for_location(&Shell::Bash, &mut "~drmorr/foo/bar".as_bytes()).unwrap_err();
}
