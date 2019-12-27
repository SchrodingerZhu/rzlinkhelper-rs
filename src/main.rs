#[macro_use]
extern crate lazy_static;

use std::io::Read;
use std::process::exit;

use log::*;
use serde::*;

use crate::cmaker::{run_cmake, run_cmaker, run_remake};
use percent_encoding::{AsciiSet, CONTROLS};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
const FRAGMENT: &AsciiSet = &CONTROLS
    .add(b' ').add(b'"').add(b'<').add(b'>').add(b'`').add(b'/').add(b'\\');
mod config;
mod compile;
mod cmaker;
mod linking;

#[derive(Deserialize, Serialize)]
struct Progress {
    cmake: bool,
    remake: bool,
    cmaker: bool,
    compile_to_llvm: bool,
    linking: bool
}

fn load_progress() -> Progress {
    match std::fs::File::open(config::PWD.clone() + "/.progress") {
        Ok(mut file) => {
            let mut buf = String::new();
            file.read_to_string(&mut buf).unwrap_or_else(|e| {
                error!("failed to read the progress file, you had better delete it and re-run: {}", e);
                exit(5)
            });
            simd_json::serde::from_str(&mut buf).unwrap_or_else(|e| {
                error!("failed to parse the progress file, you had better delete it and re-run: {}", e);
                exit(5)
            }
            )
        }
        Err(_) => match std::fs::File::create(config::PWD.clone() + "/.progress") {
            Ok(_) => {
                Progress {
                    cmake: false,
                    remake: false,
                    cmaker: false,
                    compile_to_llvm: false,
                    linking: false
                }
            }
            Err(e) => {
                error!("failed to initialize progress record: {}", e);
                exit(5);
            }
        }
    }
}

impl Drop for Progress {
    fn drop(&mut self) {
        if !self.cmake {
            if std::fs::remove_dir_all(config::PWD.clone() + "/rz_build").is_err() {
                warn!("cannot remove ./rz_build, please check if it is created");
            }
        }
        serde_json::to_string_pretty(self)
            .map_err(|e| e.into())
            .and_then(|c|std::fs::write(config::PWD.clone() + "/.progress", c))
            .unwrap_or_else(|e|
                error!("failed to store progress: {}", e)
            );
    }
}

fn main() {
    std::env::set_var("RUST_LOG", "trace");
    pretty_env_logger::init_timed();
    info!("work path: {:#?}", *config::PWD);
    info!("config file: {:#?}", *config::CONFIG);
    let build_dir = config::PWD.clone() + "/rz_build";
    let mut progress = load_progress();
    if !progress.cmake {
        run_cmake();
        progress.cmake = true;
    }
    if !progress.remake {
        std::env::set_current_dir(&build_dir).unwrap_or_else(|e|
            {
                error!("failed to change dir to ./rz_build, remove .progress if you need: {}", e);
                exit(6);
            }
        );
        run_remake();
        progress.remake = true;
    }
    if !progress.cmaker {
        std::env::set_current_dir(&build_dir).unwrap_or_else(|e|
            {
                error!("failed to change dir to ./rz_build, remove .progress if you need: {}", e);
                exit(6);
            }
        );
        run_cmaker();
        progress.cmaker = true;
    }

    let collection = cmaker::get_collection(& build_dir);
    if !progress.compile_to_llvm {
        info!("start compiling to llvm");
        compile::compile_to_llvm(&collection);
        progress.compile_to_llvm = true;
    }
    if !progress.linking {
        info!("start linking");
        linking::linking(&collection);
        progress.linking = true;
    }
    info!("all processes finished, if you want to re-run please delete the rz_build dir and the .progress file");

}
