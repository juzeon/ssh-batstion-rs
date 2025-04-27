use anyhow::bail;
use serde::de::DeserializeOwned;
use std::fs::OpenOptions;
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub static EMPTY_U8_VEC: Vec<u8> = vec![];
pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_filter(LevelFilter::DEBUG)
                .with_filter(filter_fn(|x| x.target().starts_with("ssh"))),
        )
        .with(
            fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_filter(LevelFilter::DEBUG)
                .with_filter(filter_fn(|x| x.target().starts_with("ssh"))),
        )
        .init();
}
fn file_writer() -> impl std::io::Write {
    let paths = vec!["/etc/ssh-bastion/log.log", "log.log"];
    for path in paths {
        let f = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path);
        match f {
            Ok(file) => {
                return file;
            }
            Err(_) => {
                continue;
            }
        }
    }
    panic!("cannot open log file")
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
