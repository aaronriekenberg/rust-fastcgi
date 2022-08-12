use getset::Getters;

use log::info;

use serde::{Deserialize, Serialize};

use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct ServerConfiguration {
    socket_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandInfo {
    id: String,
    description: String,
    command: String,
    args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandConfiguration {
    commands: Vec<CommandInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct Configuration {
    server_configuration: ServerConfiguration,
    command_configuration: CommandConfiguration,
}

pub async fn read_configuration(
    config_file: &str,
) -> Result<Configuration, Box<dyn std::error::Error>> {
    info!("reading {}", config_file);

    let mut file = File::open(config_file).await?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents).await?;

    let configuration: Configuration = ::serde_json::from_slice(&file_contents)?;

    info!("read_configuration configuration\n{:#?}", configuration);

    Ok(configuration)
}
