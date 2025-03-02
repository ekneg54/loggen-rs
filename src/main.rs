use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name="loggen", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to log config directory
    #[arg(short, long, value_name = "DIR")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// send logs to an http endpoint
    Http {
        /// base url of the http endpoint. Default is http://localhost:9000
        #[arg(short, long)]
        url: String,
    },

    /// send logs to a kafka topic
    Kafka {
        /// kafka config as a mapping of key value pairs
        /// e.g. "{bootstrap.servers: localhost:9092, topic: loggen}"
        /// Default is "{bootstrap.servers: localhost:9092, topic: producer}"
        #[arg(short, long)]
        kafkaconfig: String,
    },

}


fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Http { url }) => {
            println!("Sending logs to http endpoint");
        }
        Some(Commands::Kafka { kafkaconfig }) => {
            println!("Sending logs to kafka topic");
        }
        None => {
            println!("No command provided");
        }
    }
}

