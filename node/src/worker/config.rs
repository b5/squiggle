use std::{
    collections::HashMap,
    env,
    net::{IpAddr, Ipv4Addr, TcpStream},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use config::{Environment, File, Value};
use iroh::net::defaults::prod::{default_eu_relay_node, default_na_relay_node};
use iroh::net::relay::{RelayMap, RelayNode};
use iroh::node::GcPolicy;
use serde::{Deserialize, Serialize};
use tracing::{info, trace, warn};

use crate::content_routing::AutofetchPolicy;
use crate::workspace::WorkspaceConfig;

/// CONFIG_FILE_NAME is the name of the optional config file located in the iroh home directory
pub(crate) const CONFIG_FILE_NAME: &str = "fog.config.toml";

/// ENV_PREFIX should be used along side the config field name to set a config field using
/// environment variables
/// For example, `IROH_PATH=/path/to/config` would set the value of the `Config.path` field
pub(crate) const ENV_PREFIX: &str = "FOG";

/// The configuration for an iroh node.
#[derive(PartialEq, Eq, Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct NodeConfig {
    /// Host name to listen on.
    pub s3_host: String,
    /// Port number for S3 HTTP API to listen on.
    pub s3_port: u16,
    /// Access key used for S3 authentication.
    pub s3_access_key: Option<String>,
    /// Secret key used for S3 authentication.
    pub s3_secret_key: Option<String>,
    /// Domain name used for S3 virtual-hosted-style requests.
    pub s3_domain_name: Option<String>,

    /// Control automatic content fetching within a workspace
    pub autofetch_default: AutofetchPolicy,
    /// Port number for the main iroh fog HTTP API to listen on.
    pub api_port: u16,
    /// Bind address on which to serve Prometheus metrics
    pub metrics_port: Option<u16>,

    /// Port for iroh to listen on for direct connections. Defaults to 0 for random available
    /// port assignement.
    pub iroh_port: u16,
    /// The set of iroh relay nodes to use.
    pub relay_nodes: Vec<RelayNode>,
    /// How often to garbage collect blobs that have no references.
    pub gc_policy: GcPolicy,
    /// Address of the tracing collector.
    /// eg: set to http://localhost:4317 for a locally running Jaeger instance.
    pub tracing_endpoint: Option<String>,

    /// Root folder used for storing and retrieving assets shared with the worker.
    pub worker_root: PathBuf,

    /// Discord workspace name for the iroh discord bot.
    pub discord_workspace: Option<String>,
    /// Discord bot token for the iroh discord bot.
    pub discord_token: Option<String>,
    /// The domain used for accessing the bot results.
    pub discord_s3_domain: Option<String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        let worker_root =
            tempfile::TempDir::with_prefix("fog-worker").expect("unable to create tempdir");
        let worker_root = worker_root.into_path();
        Self {
            s3_host: "localhost".to_string(),
            s3_port: 8014,
            s3_access_key: Some("access".to_string()),
            s3_secret_key: Some("secret".to_string()),
            s3_domain_name: Some("localhost:8014".to_string()),
            api_port: 8015,
            metrics_port: Some(8016),
            iroh_port: 0,
            relay_nodes: [default_na_relay_node(), default_eu_relay_node()].into(),
            gc_policy: GcPolicy::Disabled,
            autofetch_default: AutofetchPolicy::Disabled,
            tracing_endpoint: None,
            discord_workspace: None,
            discord_token: None,
            worker_root,
            discord_s3_domain: None,
        }
    }
}

impl NodeConfig {
    /// Make a config from the default environment variables.
    ///
    /// Optionally provide an additional configuration source.
    pub(crate) fn load_with_env(data_root: &Path) -> anyhow::Result<Self> {
        let config_path = data_root.join(CONFIG_FILE_NAME);
        let source = match config_path.exists() {
            true => Some(config_path.as_path()),
            false => None,
        };
        let sources = [source];
        let config = Self::load_inner(
            // potential config files
            &sources,
            // env var prefix for this config
            ENV_PREFIX,
            // map of present command line arguments
            // args.make_overrides_map(),
            HashMap::<String, String>::new(),
        )?;
        Ok(config)
    }

    /// Make a config using a default, files, environment variables, and commandline flags.
    ///
    /// Later items in the *file_paths* slice will have a higher priority than earlier ones.
    ///
    /// Environment variables are expected to start with the *env_prefix*. Nested fields can be
    /// accessed using `.`, if your environment allows env vars with `.`
    ///
    /// Note: For the metrics configuration env vars, it is recommended to use the metrics
    /// specific prefix `IROH_METRICS` to set a field in the metrics config. You can use the
    /// above dot notation to set a metrics field, eg, `IROH_CONFIG_METRICS.SERVICE_NAME`, but
    /// only if your environment allows it
    fn load_inner<S, V>(
        file_paths: &[Option<&Path>],
        env_prefix: &str,
        flag_overrides: HashMap<S, V>,
    ) -> Result<NodeConfig>
    where
        S: AsRef<str>,
        V: Into<Value>,
    {
        let mut builder = config::Config::builder();

        // layer on config options from files
        for path in file_paths.iter().flatten() {
            if path.exists() {
                let p = path.to_str().ok_or_else(|| anyhow::anyhow!("empty path"))?;
                builder = builder.add_source(File::with_name(p));
            }
        }

        // next, add any environment variables
        builder = builder.add_source(
            Environment::with_prefix(env_prefix)
                .separator("__")
                .try_parsing(true),
        );

        // finally, override any values
        for (flag, val) in flag_overrides.into_iter() {
            builder = builder.set_override(flag, val)?;
        }

        let cfg = builder.build()?;
        trace!("make_config:\n{:#?}\n", cfg);
        let cfg = cfg.try_deserialize()?;
        Ok(cfg)
    }

    /// Constructs a `RelayMap` based on the current configuration.
    pub(crate) fn relay_map(&self) -> Result<Option<RelayMap>> {
        if self.relay_nodes.is_empty() {
            return Ok(None);
        }
        Some(RelayMap::from_nodes(self.relay_nodes.iter().cloned())).transpose()
    }

    pub fn ensure_open_ports(&mut self) -> Result<bool> {
        let mut any_switched = false;

        // check if api_port is open, if not, change it to a new port, update the config, and add it to open_ports
        if is_port_in_use(self.api_port) {
            let mut new_port = self.api_port;
            while is_port_in_use(new_port) || new_port == self.iroh_port {
                new_port += 1;
            }
            warn!("api_port was taken. switching to {}", new_port);
            self.api_port = new_port;
            any_switched = true;
        }

        // check if s3_port is open, if not, change it to a new port, update the config, and add it to open_ports
        if is_port_in_use(self.s3_port) {
            info!("s3_port is in use");
            let mut new_port = self.s3_port;
            while is_port_in_use(new_port)
                || new_port == self.iroh_port
                || new_port == self.api_port
            {
                new_port += 1;
            }
            warn!("s3_port was taken. switching to {}", new_port);
            self.s3_port = new_port;
            any_switched = true;
        }

        if let Some(metrics) = self.metrics_port {
            if is_port_in_use(metrics) {
                info!("metrics_port is in use");
                let mut new_port = metrics;
                while is_port_in_use(new_port)
                    || new_port == self.iroh_port
                    || new_port == self.api_port
                    || new_port == self.s3_port
                {
                    new_port += 1;
                }
                warn!("metrics_port was taken. switching to {}", new_port);
                self.metrics_port = Some(new_port);
                any_switched = true;
            }
        }

        Ok(any_switched)
    }

    pub(crate) fn workspace_config(&self) -> WorkspaceConfig {
        WorkspaceConfig {
            autofetch: self.autofetch_default.clone(),
            worker_root: self.worker_root.clone(),
        }
    }
}

/// Name of directory that wraps all fog files in a given application directory
const FOG_DIR: &str = "fog";

/// Returns the path to the user's iroh data directory.
///
/// If the `IROH_DATA_DIR` environment variable is set it will be used unconditionally.
/// Otherwise the returned value depends on the operating system according to the following
/// table.
///
/// | Platform | Value                                         | Example                                  |
/// | -------- | --------------------------------------------- | ---------------------------------------- |
/// | Linux    | `$XDG_DATA_HOME`/iroh or `$HOME`/.local/share/iroh | /home/alice/.local/share/iroh                 |
/// | macOS    | `$HOME`/Library/Application Support/iroh      | /Users/Alice/Library/Application Support/iroh |
/// | Windows  | `{FOLDERID_RoamingAppData}/iroh`              | C:\Users\Alice\AppData\Roaming\iroh           |
pub fn data_root() -> Result<PathBuf> {
    let path = if let Some(val) = env::var_os("FOG_DATA_DIR") {
        PathBuf::from(val)
    } else {
        let path = dirs_next::data_dir().ok_or_else(|| {
            anyhow!("operating environment provides no directory for application data")
        })?;
        path.join(FOG_DIR)
    };
    let path = if !path.is_absolute() {
        std::env::current_dir()?.join(path)
    } else {
        path
    };
    Ok(path)
}

pub fn is_port_in_use(port: u16) -> bool {
    TcpStream::connect((IpAddr::V4(Ipv4Addr::LOCALHOST), port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let config =
            NodeConfig::load_inner(&[][..], "__FOO", HashMap::<String, String>::new()).unwrap();

        assert_eq!(config.relay_nodes.len(), 2);
    }
}
