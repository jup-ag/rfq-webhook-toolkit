use clap::Parser;

#[derive(Clone, Parser)]
pub struct Config {
    #[clap(env, long, default_value = "0.0.0.0:8080")]
    pub listen_addr: String,

    /// set a private key for the maker here to use in the example_quote handler
    /// if it is not set, a random keypair will be generated on startup
    /// to generate a keypair, use the following command:
    /// solana-keygen new --outfile maker-keypair.json
    #[clap(env, long)]
    pub maker_keypair: Option<String>,

    #[clap(env, long, default_value = "https://api.mainnet-beta.solana.com")]
    pub rpc_url: String,
}

// Separating this so we can reuse it in tests
pub fn get_app_config() -> Config {
    dotenvy::dotenv().ok();
    Config::parse()
}
