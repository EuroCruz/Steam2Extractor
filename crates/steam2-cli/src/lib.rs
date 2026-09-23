pub mod cli;
pub mod commands;

use std::process::ExitCode;

pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        println!("{}", cli::usage());
        return ExitCode::SUCCESS;
    }

    let Some(parsed) = cli::parse(&args) else {
        eprintln!("{}", cli::usage());
        return ExitCode::from(2);
    };

    let command = match parsed {
        Ok(c) => c,
        Err(e) => {
            println!("error: {e}");
            println!("--");
            print!("{}", cli::usage());
            return ExitCode::FAILURE;
        }
    };

    let result = match &command {
        cli::Command::ExtractDepot(args) => commands::extract_depot(args),
        cli::Command::ExtractSid(args) => commands::extract_sid(args),
        cli::Command::List { path, blob_dir, dat_dir, keys, out_file } => {
            commands::list(path, blob_dir.as_deref(), dat_dir.as_deref(), keys, out_file.as_deref())
        }
        cli::Command::Verify { path, blob_dir, dat_dir, keys } => commands::verify(path, blob_dir.as_deref(), dat_dir.as_deref(), keys),
        cli::Command::Hash { text } => commands::hash(text),
        cli::Command::Version => commands::version(),
        cli::Command::Donate => commands::donate(),
        #[cfg(feature = "debug-tools")]
        cli::Command::DebugDictBin { keys_path, blobs_path, dats_path, out_dir, compression, endian } => {
            commands::debug_dictbin(keys_path, blobs_path, dats_path, out_dir, compression, endian)
        }
        #[cfg(feature = "debug-tools")]
        cli::Command::DebugDictBinRebuild { compression, endian, out_dir } => {
            commands::debug_dictbin_rebuild(compression, endian, out_dir)
        }
        #[cfg(feature = "debug-tools")]
        cli::Command::DebugDictBinLookup { kind, hash } => commands::debug_dictbin_lookup(kind, *hash),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
