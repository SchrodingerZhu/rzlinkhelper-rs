use std::env::var;
use std::io::Read;

use log::{error, info};
use serde::*;
use simd_json::serde as S;

#[derive(Deserialize, Debug)]
pub struct Configuration {
    pub callpass_library_path: String,
    pub debug: bool,
    pub original_cxx_executable: String,
    pub original_cc_executable: String,
    pub targeted_cxx_executable: String,
    pub targeted_cc_executable: String,
    pub llvm_link_executable: String,
    pub cmaker_executable: String,
    pub cmake_executable: String,
    pub remake_executable: String,
}

pub(crate) fn parse_config() -> Configuration {
    let path = var("RZ_CONFIG").unwrap_or(String::from("./config.json"));
    info!("loading config from: {}", path);
    let config = std::fs::File::open(path)
        .map_err(|x| error!("failed to open config file: {}", x))
        .and_then(|mut x| {
            let mut buf = String::new();
            x.read_to_string(&mut buf).map(|_| buf)
                .map_err(|x| error!("failed to read config file: {}", x))
        })
        .and_then(|mut x| S::from_str(x.as_mut_str())
            .map_err(|x| error!("failed to parse config file: {}", x)));
    match config {
        Ok(x) => x,
        Err(_) => {
            std::process::exit(1)
        }
    }
}

fn get_current_path() -> String {
    std::env::current_dir()
        .map_err(|e| error!("cannot get current dir {}", e))
        .and_then(|x| x.to_str()
            .ok_or_else(|| error!("cannot get current dir"))
            .map(String::from))
        .unwrap_or_else(|_| {
            error!("cannot transform current dir");
            std::process::exit(1)
        })
}

lazy_static! {
    pub static ref CONFIG : Configuration = parse_config();
    pub static ref PWD : String = get_current_path();
}

