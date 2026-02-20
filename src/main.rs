use anynode::{Config, ConfigError};

fn main() {
    match Config::from_env() {
        Ok(config) => {
            println!("AnyNode - PMTiles extraction and Storage upload tool");
            println!("Configuration loaded successfully:");
            println!("  Storage data dir: {:?}", config.storage_data_dir);
            println!("  Discovery port: {}", config.discovery_port);
            println!("  WhosOnFirst DB: {:?}", config.whosonfirst_db_path);
            println!("  CID DB: {:?}", config.cid_db_path);
            println!("  Localities dir: {:?}", config.localities_dir);
            println!("  Target countries: {:?}", config.target_countries);
            println!("  Max concurrent extractions: {}", config.max_concurrent_extractions);
            println!("  Planet PMTiles: {:?}", config.planet_pmtiles_path);
        }
        Err(ConfigError::MissingEnvVar(var)) => {
            eprintln!("Missing required environment variable: {}", var);
            eprintln!("Please set all required variables in your .env file.");
            eprintln!("See .env.example for reference.");
            std::process::exit(1);
        }
        Err(ConfigError::InvalidValue(msg)) => {
            eprintln!("Invalid configuration value: {}", msg);
            std::process::exit(1);
        }
    }
}
