use std::error::Error;

use getset::Getters;

use log::info;

use serde::{Deserialize, Serialize};

use tokio::{fs::File, io::AsyncReadExt};

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct ServerConfiguration {
    socket_path: String,
    max_concurrent_connections: u8,
    max_requests_per_connection: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandInfo {
    id: String,
    description: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandConfiguration {
    max_concurrent_commands: usize,
    semaphore_acquire_timeout_millis: u64,
    commands: Vec<CommandInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct Configuration {
    server_configuration: ServerConfiguration,
    command_configuration: CommandConfiguration,
}

pub async fn read_configuration(config_file: String) -> Result<Configuration, Box<dyn Error>> {
    info!("reading {}", config_file);

    let mut file = File::open(config_file).await?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents).await?;

    let configuration: Configuration = ::serde_json::from_slice(&file_contents)?;

    info!("read_configuration configuration\n{:#?}", configuration);

    Ok(configuration)
}
