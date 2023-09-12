use std::mem::discriminant;

use thiserror::Error;

pub type SimKubeResult<T, E = SimKubeError> = std::result::Result<T, E>;

macro_rules! sk_error_helper {
    ( {$($body:tt)*} #[$err:meta] $item:ident, $($tail:tt)*) => {
        sk_error_helper!{
            {
                $($body)*
                #[$err]
                $item,
            }
            $($tail)*
        }
    };

    ( {$($body:tt)*} #[$err:meta] $item:ident($(#[$from:meta])? $derive:ty), $($tail:tt)*) => {
        sk_error_helper!{
            {
                $($body)*
                #[$err]
                $item($(#[$from])? $derive),
            }
            $($tail)*
        }
    };

    ( {$($body:tt)*} ) => {
        #[derive(Error, Debug)]
        pub enum SimKubeError {
            $($body)*
        }

        impl PartialEq for SimKubeError {
            fn eq(&self, other: &SimKubeError) -> bool {
                return discriminant(self) == discriminant(other)
            }
        }
    };
}

macro_rules! sk_error {
    ( $($items:tt)* ) => {sk_error_helper!{{} $($items)*}};
}

sk_error! {
    #[error("config file could not be read ({0})")]
    ConfigFileError(#[from] serde_yaml::Error),

    #[error("field not present in Kubernetes object")]
    FieldNotFound,

    #[error("could not read file ({0})")]
    FileIOError(#[from] std::io::Error),

    #[error("error communicating with the apiserver ({0})")]
    KubeApiError(#[from] kube::Error),

    #[error("label selector was malformed")]
    MalformedLabelSelector,

    #[error("parse error ({0})")]
    ParseError(#[from] url::ParseError),

    #[error("error serializing trace data ({0})")]
    TraceExportError(#[from] rmp_serde::encode::Error),

    #[error("error decoding trace data ({0})")]
    TraceImportError(#[from] rmp_serde::decode::Error),

    #[error("unrecognized trace scheme: {0}")]
    UnrecognizedTraceScheme(String),
}
