use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::str::FromStr;
use nostr_rust::Identity;
use nostr_rust::nostr_client::Client;
use nostr_rust::req::ReqFilter;
use toml::{toml, Value};

#[derive(Deserialize, Serialize)]
struct Contact {
    nip05: Option<String>,
    pubkey: String,
}

type ContactList = Vec<Contact>;

#[derive(Deserialize, Serialize)]
struct Config {
    privkey: String,
    relays: Vec<String>,
    // TODO: move contact_list to a different file. its gross in toml
    contact_list: ContactList,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            privkey: "YOUR_HEX_ENCODED_PRIVKEY".to_string(),
            relays: vec!["wss://relay.damus.io".to_string()],
            contact_list: Vec::new(),
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    GenConfig,
    UpdateFollows,
    LoadFollows,
}

fn main() {
    let cli = Cli::parse();

    let config_path = cli.config.unwrap_or_else(|| {
        let mut p = env::home_dir().unwrap();
        p.push(".smart-follow");
        p.push("config.toml");
        p
    });

    match cli.command {
        Commands::GenConfig => {
            let home_dir = env::home_dir().expect("Failed to get home directory");
            let new_dir = home_dir.join(".smart-follow");
            fs::create_dir(&new_dir);
            let fname = new_dir.join("config.toml");
            let mut file = fs::File::create(&fname).expect(&format!(
                "Could not create config file at {}",
                fname.to_str().unwrap()
            ));
            file.write_all(toml::to_string(&Config::default()).unwrap().as_bytes())
                .expect("Could not write config file");
            println!("Empty config file written to {}", fname.to_str().unwrap());
        }
        Commands::LoadFollows => {
            // TODO: this will overwrite the contact list in the config. make that an option.
            let mut config: Config = toml::from_str(&fs::read_to_string(&config_path).expect(
                "Could not open config file. check your path or run the `gen-config` command",
            ))
            .expect("Could not parse config file.");

            config.contact_list = Vec::new();

            let identity = Identity::from_str(&config.privkey).expect("Could not load secret key from config. make sure it's hex-encoded");
            let relays: Vec<&str> = config.relays.iter().map(|relay| relay.borrow()).collect();
            let mut client = Client::new(relays).expect("Could not create client to relays");

            println!("Attempting to fetch contact list events from relays");
            let contact_lists = client.get_events_of(vec![ReqFilter {
                ids: None,
                authors: Some(vec![identity.public_key_str]),
                kinds: Some(vec![3]),
                e: None,
                p: None,
                since: None,
                until: None,
                limit: None,
            }]).expect("Could not fetch kind-3 events from relays");

            let mut contact_pubkeys = HashSet::new();
            for cl in contact_lists {
                for tag in cl.tags {
                    if tag.first().unwrap() == "p" {
                        if let Some(k) = tag.get(1) {
                            contact_pubkeys.insert(k.to_string());
                        }
                    } else {continue;}
                }
            }

            println!("Got an aggregate contact list of {} follows. Writing to config file.", contact_pubkeys.len());
            for pubkey in contact_pubkeys {
                let c = Contact {
                    pubkey,
                    nip05: None,
                };
                config.contact_list.push(c);
            }
            let mut f = fs::File::create(&config_path).expect("could not open config file for writing");
            f.write_all(toml::to_string(&config).unwrap().as_bytes()).expect("Could not write config to file");

            println!("Going to try resolving contacts to NIP05 identifiers...")
            for mut contact in config.contact_list {

            }

        }
        Commands::UpdateFollows => {
            todo!()
        }
    }
}
