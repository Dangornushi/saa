mod models;
mod llm;
mod scheduler;
mod storage;
mod cli;
mod calendar;
mod config;

use cli::{Cli, CliApp};
use anyhow::Result;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let use_mock_llm = cli.mock_llm;
    let verbose = cli.verbose;
    
    let mut app = CliApp::new(use_mock_llm, verbose)?;
    app.run(cli)?;
    
    Ok(())
}
