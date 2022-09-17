#![warn(rust_2018_idioms)]

use anyhow::Context;

use log::error;

use tikv_jemallocator::Jemalloc;

mod config;
mod handlers;
mod request;
mod response;
mod server;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

async fn try_main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_nanos().init();

    let config_file = std::env::args()
        .nth(1)
        .context("config file required as command line argument")?;

    let configuration = crate::config::read_configuration(config_file)
        .await
        .context("read_configuration error")?;

    let handlers =
        crate::handlers::create_handlers(&configuration).context("create_handlers error")?;

    let server = crate::server::Server::new(handlers, configuration.server_configuration());

    server.run().await.context("server.run error")?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = try_main().await {
        error!("main got fatal error:\n{:#}", err);
        std::process::exit(1);
    }
}
