use anyhow::Result;
use std::env;

use super::cli;

pub(super) struct App;

impl App {
    pub(super) fn new() -> App {
        App {}
    }

    pub(super) async fn execute(&self) -> Result<()> {
        cli::Cli::execute(env::args()).await
    }
}
