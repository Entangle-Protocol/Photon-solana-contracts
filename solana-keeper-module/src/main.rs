mod cli;
mod listener_app;
mod logging;

extern crate photon;

use std::env;

#[tokio::main]
async fn main() {
    logging::init_logging();
    cli::Cli::execute(env::args()).await;
}
