use anyhow::bail;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                // .with_writer(std::io::stderr)
                .with_filter(LevelFilter::DEBUG)
                .with_filter(filter_fn(|x| x.target().starts_with("ssh"))),
        )
        .init()
}
pub fn load_config<T: DeserializeOwned>() -> anyhow::Result<T> {
    let path_arr = vec![
        "config.yml",
        "config.yaml",
        "/etc/ssh-bastion/config.yml",
        "/etc/ssh-bastion/config.yaml",
    ];
    for path in path_arr {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                info!(path, "Load config");
                let res = serde_yaml::from_str::<T>(&s)?;
                return Ok(res);
            }
            Err(_) => continue,
        }
    }
    bail!("Cannot find config file")
}
