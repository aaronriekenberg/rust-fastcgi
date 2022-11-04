#![warn(rust_2018_idioms)]

use anyhow::Context;

use log::error;

mod config;
mod connection;
mod handlers;
mod request;
mod response;
mod server;
mod utils;

fn app_name() -> String {
    std::env::args().nth(0).unwrap_or("[UNKNOWN]".to_owned())
}

async fn try_main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_nanos().init();

    let config_file = std::env::args().nth(1).with_context(|| {
        format!(
            "config file required as command line argument: {} <config file>",
            app_name(),
        )
    })?;

    crate::config::read_configuration(config_file)
        .await
        .context("read_configuration error")?;

    let handlers = crate::handlers::create_handlers().context("create_handlers error")?;

    let server = crate::server::Server::new(handlers);

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
