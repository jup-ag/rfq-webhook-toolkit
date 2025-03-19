mod config;
mod server;

use config::get_app_config;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[async_std::main]
async fn main() {
    // setup tracing
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter)
        .try_init()
        .unwrap();

    let config = get_app_config();
    server::serve(config).await;
}
