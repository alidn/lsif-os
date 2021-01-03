use std::env;

use anyhow::Context;
use cli::Opts;
use indicatif::ProgressBar;
use structopt::StructOpt;

use crate::{emitter::file_emitter::FileEmitter, indexer::indexer::Indexer};

mod analyzer;
mod cli;
mod emitter;
pub mod indexer;
mod language_tools;
mod protocol;

fn main() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let args = env::args();
    // A hack to avoid sub-commands
    for arg in args {
        if &arg == "--langs" {
            println!("Currently supported languages:");
            println!("\t- JavaScript");
            println!("\t- GraphQL");
            println!("\t- Java");
            println!("\t- TypeScript");
            return;
        }
    }

    let start = std::time::Instant::now();

    let spinner = ProgressBar::new_spinner();
    spinner.set_message("Parsing files");

    let mut opt: Opts = Opts::from_args();
    opt.canonicalize_paths();

    let output = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&opt.output.clone().unwrap())
        .context("Could not open the output file")
        .unwrap();
    output.set_len(0).unwrap();

    let (emitter, signal_receiver) = FileEmitter::new(output);

    Indexer::index(opt, emitter).unwrap();

    spinner.enable_steady_tick(60);
    spinner.set_message("waiting for the buffer to be flushed");

    // Wait until the buffer is flushed
    signal_receiver.recv().unwrap();

    spinner.finish_with_message(&format!(
        "Finished indexing, took {}ms",
        start.elapsed().as_millis()
    ));
}
