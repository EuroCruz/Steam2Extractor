use std::process::ExitCode;

use extractcore::{cli, extract, sid, sidcli};

fn usage(prog: &str) -> String {
    format!(
        "usage: {prog} depot version [options]\n       {prog} file.sim [file2.sim ...] [options]\n\n\
         depot             depot id\n\
         version           version\n\
         file.sim          .sim manifest, one per disk, in order\n\n\
         --blob-dir dir    blob directory, downloaded automatically (default: {})\n\
         --dat-dir dir     dat directory, downloaded automatically (default: {})\n\
         --blobcrc crc     blob crc, only needed after a depot reset\n\
         --key key         depot key, hex or depot:hex, repeatable\n\
         --keys-file file  \"depot\" \"key\" pairs, one per line, repeatable\n\
         --filter regex    only extract matching paths\n\
         --out dir         output directory\n\
         --offline         no network, local files only\n\n\
         {prog} donate     show donation addresses\n",
        cli::DEFAULT_BLOB_DIR,
        cli::DEFAULT_DAT_DIR
    )
}

fn donate() {
    println!("bitcoin        bc1q2e60ws5yy6m5czv5wtc97z28vp8et3quvzx35c");
    println!("solana         HgqXV2YMWQggDJNDWzJ3rrczwhhy9AAUq7xZcziBqHbC");
    println!("eth/base/bnb   0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e");
}

fn is_sid_mode(argv: &[String]) -> bool {
    argv.iter()
        .any(|a| !a.starts_with("--") && a.to_ascii_lowercase().ends_with(".sim"))
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv
        .first()
        .cloned()
        .unwrap_or_else(|| "extract".to_string());

    if argv.len() == 1 {
        print!("{}", usage(&prog));
        return ExitCode::SUCCESS;
    }

    if argv[1] == "donate" {
        donate();
        return ExitCode::SUCCESS;
    }

    if is_sid_mode(&argv[1..]) {
        let args = match sidcli::parse(&argv[1..]) {
            Ok(args) => args,
            Err(err) => {
                println!("error: {}", err);
                println!("--");
                print!("{}", usage(&prog));
                return ExitCode::FAILURE;
            }
        };
        if let Err(err) = sid::run(&args) {
            eprintln!("error: {}", err);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    let args = match cli::parse(&argv[1..]) {
        Ok(args) => args,
        Err(err) => {
            println!("error: {}", err);
            println!("--");
            print!("{}", usage(&prog));
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = extract::run(&args) {
        eprintln!("error: {}", err);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
