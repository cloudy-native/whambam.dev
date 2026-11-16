// SPDX-License-Identifier: MIT

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    whambam::run(whambam::cli::Cli::parse()).await
}
