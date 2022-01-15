use std::{collections::HashMap, fs, io::Write};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct UrlCache {
    pub url: String,
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub favicon: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Cache {
    pub urls: HashMap<String, UrlCache>,
    pub contents: HashMap<String, Vec<u8>>,
}

pub fn get_cache(path: &str) -> Result<Cache, ()> {
    fs::read_to_string(path)
        .map_err(|_| ())
        .map(|v| serde_json::from_str::<Cache>(&v).map_err(|_| ()))?
}

pub fn write_cache(
    path: &str,
    urls: HashMap<String, UrlCache>,
    contents: HashMap<String, Vec<u8>>,
) -> Result<(), ()> {
    let cache = Cache { urls, contents };
    match fs::File::create(path) {
        Ok(mut v) => v
            .write_all(serde_json::to_string(&cache).map_err(|_| ())?.as_bytes())
            .map_err(|_| ()),
        _ => Err(()),
    }
}
