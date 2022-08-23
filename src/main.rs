mod config;
mod handlers;
mod request;
mod response;
mod server;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().format_timestamp_nanos().init();

    let config_file = std::env::args()
        .nth(1)
        .ok_or("config file required as command line argument")?;

    let configuration = config::read_configuration(config_file).await?;

    let server = crate::server::Server::new(configuration);
    server.run().await?;

    Ok(())
}
