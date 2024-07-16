use std::fs::File;
use std::io::{
    BufRead,
    Write,
};
use std::path::{
    Path,
    PathBuf,
};
use std::{
    fs,
    io,
};

use anyhow::anyhow;
use clap::value_parser;
use clap_complete::{
    generate,
    Shell,
};
use sk_core::prelude::*;

#[derive(clap::Args)]
pub struct Args {
    #[arg(
        long_help = "name of the shell to generate completion files for",
        value_parser = value_parser!(clap_complete::Shell),
    )]
    pub shell: Shell,

    #[arg(short = 'o', long = "stdout", long_help = "print to stdout")]
    pub stdout: bool,
}

pub(super) fn default_path_for(shell: &Shell) -> PathBuf {
    let mut default_path = dirs::data_dir().unwrap_or(PathBuf::from("."));
    match shell {
        Shell::Bash => default_path.push("bash-completion"),
        Shell::Elvish => default_path.push("elvish/lib"),
        Shell::Fish => default_path.push("fish/vendor_completions.d"),
        Shell::PowerShell => default_path.push("SimKube"),
        Shell::Zsh => default_path.push("zsh/site-functions"),
        _ => (),
    };
    default_path
}

fn completion_filename_for(shell: &Shell) -> &'static str {
    match shell {
        Shell::Bash => "skctl",
        Shell::Elvish => "skctl.elv",
        Shell::Fish => "skctl.fish",
        Shell::PowerShell => "TabCompletions.ps1",
        Shell::Zsh => "_skctl",
        _ => "_skctl",
    }
}

pub(super) fn prompt_for_location(shell: &Shell, input: &mut impl BufRead) -> anyhow::Result<PathBuf> {
    let default_path = default_path_for(shell);
    println!("Where to install completions file? enter for default ({})", default_path.to_string_lossy());

    let pathname = input.lines().next().ok_or(anyhow!("could not read stdin"))??;
    let mut path = if pathname == String::new() {
        default_path
    } else if pathname.starts_with('~') {
        let stripped_pathname = pathname
            .strip_prefix("~/")
            .ok_or(anyhow!("computing other user homedirs unsupported"))?;
        let mut p = dirs::home_dir().ok_or(anyhow!("could not compute home dir"))?;
        p.push(stripped_pathname);
        p
    } else {
        PathBuf::from(pathname)
    };

    path.push(completion_filename_for(shell));
    Ok(path)
}

fn print_extra_info(shell: &Shell, path: &Path) {
    match shell {
        Shell::Elvish => println!("Now add `use skctl` to your `rc.elv`"),
        Shell::PowerShell => println!("Now add `. {}` to your $PROFILE script", path.to_string_lossy()),
        Shell::Zsh => {
            println!(
                "You may need to add {} to $fpath in your .zshrc",
                path.parent().expect("should be a parent").to_string_lossy()
            )
        },
        _ => (),
    }
}

pub fn cmd(args: &Args, mut cmd: clap::Command) -> EmptyResult {
    let (mut out, maybe_path): (Box<dyn Write>, Option<PathBuf>) = if args.stdout {
        (Box::new(io::stdout()), None)
    } else {
        let path = prompt_for_location(&args.shell, &mut io::stdin().lock())?;
        fs::create_dir_all(path.parent().expect("should be a parent"))?;
        (Box::new(File::create(&path)?), Some(path))
    };

    generate(args.shell, &mut cmd, "skctl", &mut out);

    if let Some(path) = maybe_path {
        println!("Completions written to {}", path.to_string_lossy());
        print_extra_info(&args.shell, &path);
        println!("You may need to restart your shell.");
    }

    Ok(())
}
