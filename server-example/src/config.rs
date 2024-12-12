use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
pub struct Config {
    #[clap(env, long, default_value = "0.0.0.0:8080")]
    pub listen_addr: String,

    // this is a example private key for testing purposes
    // NEVER USE THIS KEY OUTSIDE OF TESTING
    #[clap(
        env,
        long,
        //
        default_value = "3kUgMPkZYy65ojZBphF9WZBP1VwDqPmdPiLfNdoq7Sme"
    )]
    pub maker_private_key: String,

    #[clap(
        env,
        long,
        default_value = "5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc"
    )]
    pub maker_address: String,
}

// Separating this so we can reuse it in tests
pub fn get_app_config() -> Arc<Config> {
    dotenvy::dotenv().ok();
    Arc::new(Config::parse())
}
