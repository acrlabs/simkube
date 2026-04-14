use insta::assert_snapshot;

use super::*;
use crate::validation::{
    VALIDATORS,
    write_summary,
};

mod itest {
    use super::*;

    #[rstest]
    fn test_validate_all_rules() {
        let trace = sk_testutils::exported_trace_from_json("validation_trace");
        let mut validators = VALIDATORS.lock().unwrap();
        let failed_checks = validators.validate_trace(&trace).unwrap();
        let mut snapshot: Vec<u8> = Vec::new();
        write_summary(&mut snapshot, "validation_trace.json", &validators, failed_checks, true).unwrap();
        assert_snapshot!(str::from_utf8(&snapshot).unwrap());
    }
}
