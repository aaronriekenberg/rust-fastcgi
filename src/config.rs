use anyhow::Context;

use getset::Getters;

use log::info;

use serde::{Deserialize, Serialize};

use tokio::{fs::File, io::AsyncReadExt, sync::OnceCell};

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct ContextConfiguration {
    context: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum ServerType {
    TCP,
    UNIX,
}

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct FastCGIConnectionConfiguration {
    max_concurrent_connections: u8,
    max_requests_per_connection: u8,
}

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct ServerConfiguration {
    server_type: ServerType,
    bind_address: String,
    fastcgi_connection_configuration: FastCGIConnectionConfiguration,
}

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandInfo {
    id: String,
    description: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CommandConfiguration {
    max_concurrent_commands: usize,
    semaphore_acquire_timeout_millis: u64,
    commands: Vec<CommandInfo>,
}

#[derive(Debug, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct Configuration {
    context_configuration: ContextConfiguration,
    server_configuration: ServerConfiguration,
    command_configuration: CommandConfiguration,
}

static CONFIGURATION_INSTANCE: OnceCell<Configuration> = OnceCell::const_new();

pub async fn read_configuration(config_file: String) -> anyhow::Result<()> {
    info!("reading '{}'", config_file);

    let mut file = File::open(&config_file)
        .await
        .with_context(|| format!("error opening '{}'", config_file))?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents)
        .await
        .with_context(|| format!("error reading '{}'", config_file))?;

    let configuration: Configuration = ::serde_json::from_slice(&file_contents)
        .with_context(|| format!("error unmarshalling '{}'", config_file))?;

    info!("configuration\n{:#?}", configuration);

    CONFIGURATION_INSTANCE
        .set(configuration)
        .context("CONFIGURATION_INSTANCE.set error")?;

    Ok(())
}

pub fn get_configuration() -> &'static Configuration {
    CONFIGURATION_INSTANCE.get().unwrap()
}
