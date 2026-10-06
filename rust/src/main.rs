// Author: Daniel Hallman

use clap::Parser;
use fetch_captions::{
    fetch_captions, plain_lines, timed_lines, CaptionPayload,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "fetch-captions", about = "Fetch YouTube captions as JSON or text files")]
struct Args {
    /// YouTube URL or 11-character video id
    url: String,

    /// Write JSON to this path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Write json, timed, and plain files here
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Write a [M:SS] transcript here
    #[arg(long)]
    timed: Option<PathBuf>,

    /// Write caption text without timestamps
    #[arg(long)]
    plain: Option<PathBuf>,

    /// Preferred caption language (default: en)
    #[arg(short, long, default_value = "en")]
    language: String,
}

fn write_dir(payload: &CaptionPayload, directory: &Path) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;
    let dumped = serde_json::to_string_pretty(payload).map_err(std::io::Error::other)? + "\n";
    fs::write(directory.join("transcript.json"), dumped)?;
    fs::write(directory.join("transcript-timed.txt"), timed_lines(payload))?;
    fs::write(directory.join("transcript.txt"), plain_lines(payload))?;
    Ok(())
}

fn main() -> ExitCode {
    let args = Args::parse();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .unwrap_or_default();

    let payload = match fetch_captions(&client, &args.url, &args.language) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let dumped = match serde_json::to_string_pretty(&payload) {
        Ok(s) => s + "\n",
        Err(e) => {
            eprintln!("error formatting json: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(dir) = &args.dir {
        if let Err(e) = write_dir(&payload, dir) {
            eprintln!("error writing dir: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", dir.display());
    }

    if let Some(out) = &args.output {
        if let Err(e) = fs::write(out, &dumped) {
            eprintln!("error writing output: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", out.display());
    }

    if let Some(timed) = &args.timed {
        if let Err(e) = fs::write(timed, timed_lines(&payload)) {
            eprintln!("error writing timed: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", timed.display());
    }

    if let Some(plain) = &args.plain {
        if let Err(e) = fs::write(plain, plain_lines(&payload)) {
            eprintln!("error writing plain: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {}", plain.display());
    }

    if args.dir.is_none() && args.output.is_none() && args.timed.is_none() && args.plain.is_none() {
        print!("{dumped}");
    }

    ExitCode::SUCCESS
}
