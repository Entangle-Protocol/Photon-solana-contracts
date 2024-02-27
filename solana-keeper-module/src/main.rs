mod app;
mod cli;
mod logging;
mod solana_listener;

extern crate photon;

#[tokio::main]
async fn main() {
    logging::init_logging();
    let app = app::App::new();
    let _ = app.execute().await;
}
