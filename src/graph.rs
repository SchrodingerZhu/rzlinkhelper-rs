use std::io::{BufRead, ErrorKind};
use std::process::{exit, Stdio};
use rayon::prelude::*;
use log::*;
use percent_encoding::percent_encode;

use crate::cmaker::Collection;

pub(crate) fn gen_graph(collection: &Collection) {
    let a = crate::config::PWD.clone() + "/rz_build/graph";
    if std::fs::metadata(&a).is_err() {
        std::fs::create_dir(&a).unwrap_or_else(|x| {
            error!("unable to create dir {}: {:#?}", a, x);
            exit(100);
        })
    }
    collection.scripts.par_iter().filter(|x| x.target.target_type < 2).for_each(|x| {
        let m = x.target.abs_path.as_str();
        let encoded = percent_encode(m.as_bytes(), crate::FRAGMENT).to_string();
        let path = crate::config::PWD.clone() + "/rz_build/objects/" + encoded.as_str();
        let output = a.clone() + "/" + encoded.as_str();
        if std::fs::metadata(&output).is_err() {
            std::process::Command::new(&crate::config::CONFIG.llvm_opt_executable)
                .arg("-load")
                .arg(&crate::config::CONFIG.callpass_library_path)
                .arg("-dumpcalls")
                .arg(&path)
                .env("CALLGRAPH_STORE", &output)
                .stderr(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()
                .and_then(|mut x| {
                    let p = x.stderr.take().unwrap();
                    let reader = std::io::BufReader::new(p);
                    for i in reader.lines().map(|x| x.unwrap()) {
                        warn!("message from {}:\n {}", &path, i);
                    }
                    match x.wait() {
                        Ok(e) if e.success() => Ok(()),
                        Ok(e) => {
                            Err(std::io::Error::new(ErrorKind::Other, format!("failed with {:#?}", e)))
                        }
                        Err(e) => Err(e)
                    }
                }).unwrap_or_else(|e| {
                error!("failed to gen callgraph for {}: {}", m, e);
            });
        } else {
            info!("found {}, using cached", output);
        }
    });
}
