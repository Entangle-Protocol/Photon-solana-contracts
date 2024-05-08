mod cli;
mod common;
mod executor_app;
mod listener_app;

extern crate photon;

use std::env;

#[tokio::main]
async fn main() {
    env_logger::init();
    cli::Cli::execute(env::args()).await;
}
