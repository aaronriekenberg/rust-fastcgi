mod config;
mod handlers;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().format_timestamp_nanos().init();

    let configuration = config::read_configuration("config.json").await?;

    crate::server::run_server(configuration).await
}
