use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use nostr_rust::Identity;
use nostr_rust::nostr_client::Client;
use nostr_rust::req::ReqFilter;
use toml::{toml, Value};

#[derive(Deserialize, Serialize)]
struct ContactList {
    nip05_contacts: HashMap<String, String>, // nip05 -> pubkey
    unwashed_masses: HashSet<String>, // pubkey
}

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
            contact_list: ContactList {
                nip05_contacts: HashMap::new(),
                unwashed_masses: HashSet::new(),
            },
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
        p.push("config.json");
        p
    });

    match cli.command {
        Commands::GenConfig => {
            let home_dir = env::home_dir().expect("Failed to get home directory");
            let new_dir = home_dir.join(".smart-follow");
            fs::create_dir(&new_dir);
            let fname = new_dir.join("config.json");
            let mut file = fs::File::create(&fname).expect(&format!(
                "Could not create config file at {}",
                fname.to_str().unwrap()
            ));
            println!("{}", serde_json::to_string(&Config::default()).unwrap());
            file.write_all(serde_json::to_string(&Config::default()).unwrap().as_bytes())
                .expect("Could not write config file");
            println!("Empty config file written to {}", fname.to_str().unwrap());
        }
        Commands::LoadFollows => {
            let mut config: Config = serde_json::from_str(&fs::read_to_string(&config_path).expect(
                "Could not open config file. check your path or run the `gen-config` command",
            ))
                .expect("Could not parse config file.");

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

            // TODO: this will overwrite the contact list in the config. make that an option.
            config.contact_list.unwashed_masses = HashSet::new();
            for cl in contact_lists {
                for tag in cl.tags {
                    if tag.first().unwrap() == "p" {
                        if let Some(k) = tag.get(1) {
                            config.contact_list.unwashed_masses.insert(k.to_string());
                        }
                    } else { continue; }
                }
            }

            println!("Got an aggregate contact list of {} follows. Writing to config file.", config.contact_list.unwashed_masses.len());
            {
                let mut f = fs::File::create(&config_path).expect("could not open config file for writing");
                f.write_all(serde_json::to_string(&config).unwrap().as_bytes()).expect("Could not write config to file");
            }
            println!("Going to try resolving contacts to NIP05 identifiers...");

            // TODO: this will overwrite the contact list in the config. make that an option.
            config.contact_list.nip05_contacts = HashMap::new();
            let metadata_events = client.get_events_of(vec![ReqFilter {
                ids: None,
                authors: Some(config.contact_list.unwashed_masses.clone().into_iter().collect()),
                kinds: Some(vec![0]),
                e: None,
                p: None,
                since: None,
                until: None,
                limit: None,
            }]).expect("Could not fetch kind-0 (metadata) events from relays");
            let mut latest_ts: HashMap<String, u64> = HashMap::new();
            for event in metadata_events {
                if let Ok(content) = serde_json::from_str::<serde_json::Value>(&event.content) {
                    if let Some(content) = content.as_object() {
                        if let Some(nip05) = content.get("nip05") {
                            let latest_update = latest_ts.get(&event.pub_key).unwrap_or(&0);
                            if event.created_at > *latest_update {
                                latest_ts.insert(event.pub_key.clone(), event.created_at);
                                config.contact_list.nip05_contacts.insert(nip05.as_str().unwrap().to_string(), event.pub_key.clone());
                                config.contact_list.unwashed_masses.remove(&event.pub_key);
                            }
                        }
                    }
                }
            }
            println!("Managed to find NIP05 identifiers for {} entries! Going to write to disk", config.contact_list.nip05_contacts.len());
            {
                let mut f = fs::File::create(&config_path).expect("could not open config file for writing");
                f.write_all(serde_json::to_string(&config).unwrap().as_bytes()).expect("Could not write config to file");
            }

        }
        Commands::UpdateFollows => {
            todo!()
        }
    }
}
