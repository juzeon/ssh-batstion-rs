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
