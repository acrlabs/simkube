pub use std::backtrace::Backtrace;

pub use anyhow::{anyhow, bail, ensure};
pub use paste::paste;
pub use regex::{Regex, RegexBuilder};
pub use thiserror::Error;

pub type EmptyResult = anyhow::Result<()>;

pub const BUILD_DIR: &str = "/.build/";
pub const RUSTC_DIR: &str = "/rustc/";
pub const GLIBC: &str = "glibc";

// This macro creates an enum which derives from thiserror::Error, and also
// creates constructor functions in snake case for each of the enum variants
#[macro_export]
macro_rules! err_impl {
    (@hidden $errtype:ident, $item:ident, String) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &str) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.into())}
            }
        }
    };

    (@hidden $errtype:ident, $item:ident, $($dtype:tt)::+) => {
        paste! {
            pub(crate) fn [<$item:snake>](in_: &$($dtype)::+) -> anyhow::Error {
                anyhow!{$errtype::$item(in_.clone())}
            }
        }
    };

    ($errtype:ident,
        $(#[$errinfo:meta] $item:ident($($dtype:tt)::+),)+
    ) => {
        #[derive(Debug, Error)]
        pub(crate) enum $errtype {
            $(#[$errinfo] $item($($dtype)::+)),+
        }

        impl $errtype {
            $(err_impl! {@hidden $errtype, $item, $($dtype)::+})+
        }
    };
}

// This unholy mess prunes down a 70-plus tokio-laden backtrace into _just_ the
// bits of the backtrace that are relevant to our code.  It's _heavily_ modified from
// https://github.com/rust-lang/rust/issues/79676#issuecomment-1502670961.
//
// It's also reasonably expensive to call, which I more-or-less justify since it should
// only be called in exceptional circumstances, but if it's getting called regularly it
// potentially maybe could bog things down?
//
// It's also also probably fairly brittle, so there's a non-zero chance that important
// stack frames will get pruned.
#[macro_export]
macro_rules! skerr {
    (@hidden $err:ident, $msg:literal, $($args:expr),*) => {
        let bt = $err.backtrace().to_string();
        let re = RegexBuilder::new(r"^\s+\d+(?s:.*?)(\s+at\s+.*:\d+)$")
            .multi_line(true)
            .build()
            .unwrap();
        let mut skipped_frames = 0;
        let mut filtered_bt = re.find_iter(&bt).fold(String::new(), |mut acc, frame| {
            let frame = frame.as_str();
            if frame.contains(BUILD_DIR) || frame.contains(RUSTC_DIR) || frame.contains (GLIBC) {
                skipped_frames += 1;
            } else if !frame.is_empty() {
                if skipped_frames == 1 {
                    acc += &format!("      -- <skipped 1 frame> --\n");
                } else if skipped_frames > 1 {
                    acc += &format!("      -- <skipped {skipped_frames} frames> --\n");
                }
                acc += &format!("{frame}\n");
                skipped_frames = 0;
            }
            acc
        });

        if skipped_frames == 1 {
            filtered_bt += &format!("      -- <skipped 1 frame> --");
        } else if skipped_frames > 1 {
            filtered_bt += &format!("      -- <skipped {skipped_frames} frames> --");
        }
        error!(concat!($msg, "\n\n{}\n\nPartial Stack Trace:\n\n{}\n\n") $(, $args)*, $err, filtered_bt);
    };

    ($err:ident, $msg:literal) => {
        skerr! {@hidden $err, $msg, };
    };

    ($err:ident, $msg:literal, $($args:expr),*) => {
        skerr! {@hidden $err, $msg, $($args),*};
    };
}

pub use {err_impl, skerr};
