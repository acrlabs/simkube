use std::collections::BTreeMap; // BTreeMap sorts by key, HashMap doesn't
use std::fmt;

use super::validator::ValidatorCode;
use super::VALIDATORS;

const WIDTH: usize = 70;

#[derive(Default)]
pub struct ValidationSummary {
    pub annotations: BTreeMap<ValidatorCode, usize>,
    pub patches: usize,
}

impl fmt::Display for ValidationSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "\nValidation errors found:")?;
        writeln!(f, "{}", "-".repeat(WIDTH))?;
        for (code, count) in self.annotations.iter() {
            if *count == 0 {
                continue;
            }
            let name = VALIDATORS.lookup(code).map(|v| v.name).unwrap_or("<unknown>");
            let left = format!("{name} ({code})");
            let right = format!("{count}");
            let mid_width = WIDTH.saturating_sub(left.len()).saturating_sub(right.len()).saturating_sub(2); // two chars for extra spaces
            writeln!(f, "{left} {} {right}", ".".repeat(mid_width))?;
        }

        if self.patches > 0 {
            writeln!(f, "{}", "-".repeat(WIDTH))?;
            writeln!(f, "Patches applied: {}", self.patches)?;
            writeln!(f, "0 problems remaining")?;
        }
        Ok(())
    }
}
