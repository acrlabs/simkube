pub use std::backtrace::Backtrace;
use std::ops::Deref;

pub use anyhow::{
    anyhow,
    bail,
    ensure,
};
pub use paste::paste;
pub use regex::{
    Regex,
    RegexBuilder,
};
pub use thiserror::Error;

pub type EmptyResult = anyhow::Result<()>;

pub const BUILD_DIR: &str = "/.build/";
pub const RUSTC_DIR: &str = "/rustc/";
pub const GLIBC: &str = "glibc";

// This is sortof a stupid hack, because anyhow::Error doesn't derive from
// std::error::Error, but the reconcile functions require you to return a
// result that derives from std::error::Error.  So we just wrap the anyhow,
// and then implement deref for it so we can get back to the underlying error
// wherever we actually care.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct AnyhowError(#[from] anyhow::Error);

impl Deref for AnyhowError {
    type Target = anyhow::Error;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
        let re = RegexBuilder::new(r"^\s+(\d+)(?s:.*?)\s+at\s+.*:\d+$")
            .multi_line(true)
            .build()
            .unwrap();
        let bad_frame_index: i32 = -2;

        // Frame indices start at 0 so set this to -1 so the math works out
        let mut last_frame_index: i32 = -1;
        let mut frame_index: i32 = 0;
        let mut filtered_bt = re.captures_iter(&bt).fold(String::new(), |mut acc, frame_capture| {
            // 0th capture group guaranteed to be not none, so unwrap is safe
            let frame = frame_capture.get(0).unwrap().as_str();
            let skipped_frames = frame_index - last_frame_index;
            if !(frame.contains(BUILD_DIR) || frame.contains(RUSTC_DIR) || frame.contains (GLIBC)) && !frame.is_empty() {
                frame_index = str::parse::<i32>(frame_capture.get(1).map_or("", |m| m.as_str())).unwrap_or(bad_frame_index);
                // subtract one so adjact frames don't skip
                let skipped_frame_count = frame_index - last_frame_index - 1;
                let skipped_frames = if frame_index == bad_frame_index || last_frame_index == bad_frame_index {
                    acc += &format!("      -- <skipped unknown frames> --\n");
                } else if skipped_frame_count == 1 {
                    acc += &format!("      -- <skipped 1 frame> --\n");
                } else if skipped_frame_count > 1 {
                    acc += &format!("      -- <skipped {skipped_frame_count} frames> --\n");
                };
                acc += &format!("{frame}\n");
                last_frame_index = frame_index;
            }
            acc
        });

        let skipped_frame_count = frame_index - last_frame_index - 1;
        let skipped_frames = if frame_index == bad_frame_index || last_frame_index == bad_frame_index {
            filtered_bt += &format!("      -- <skipped unknown frames> --");
        } else if skipped_frame_count == 1 {
            filtered_bt += &format!("      -- <skipped 1 frame> --");
        } else if skipped_frame_count > 1 {
            filtered_bt += &format!("      -- <skipped {skipped_frame_count} frames> --");
        };
        error!(concat!($msg, "\n\n{}\n\nPartial Stack Trace:\n\n{}\n\n") $(, $args)*, $err, filtered_bt);
    };

    ($err:ident, $msg:literal) => {
        skerr! {@hidden $err, $msg, };
    };

    ($err:ident, $msg:literal, $($args:expr),*) => {
        skerr! {@hidden $err, $msg, $($args),*};
    };
}

pub use {
    err_impl,
    skerr,
};
