use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use std::io;
use serenity::model::prelude::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub ticket_category_id: Vec<u64>,
    pub allowed_channel_id: u64,
    pub allowed_ticket_cat_id: u64,
    pub mod_roles: Vec<u64>,
}

pub fn load_config<P: AsRef<Path>>(path: P) -> io::Result<Config> {
    let config_str = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&config_str)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    Ok(config)
}

pub fn get_mod_roles(config: &Config) -> Vec<RoleId> {
    config.mod_roles.iter().map(|&id| RoleId::new(id)).collect()
}