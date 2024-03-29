use std::fs;
use std::io::{self, BufWriter};
use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use kube::CustomResourceExt;
use shardlabel::api::ShardingRule;

#[derive(Parser)]
struct Args {
    /// Path to write CRD to.
    output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let crd = ShardingRule::crd();

    match args.output {
        Some(path) if path.extension().is_some_and(|ext| ext == "json") => {
            let file = fs::File::create(path).context("open file for writing")?;
            serde_json::to_writer(BufWriter::new(file), &crd).context("writing CRD to file")?;
        }
        Some(path) => {
            let file = fs::File::create(path).context("open file for writing")?;
            serde_yaml::to_writer(BufWriter::new(file), &crd).context("writing CRD to file")?;
        }
        None => {
            serde_yaml::to_writer(BufWriter::new(io::stdout()), &crd)
                .context("printing CRD to stdout")?;
        }
    }

    Ok(())
}
