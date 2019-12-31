use std::io::ErrorKind;
use std::process::exit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::*;
use percent_encoding::percent_encode;
use rayon::prelude::*;

use crate::cmaker::Collection;
use crate::config::*;

pub fn compile_to_llvm(collection: &Collection) {
    let commands = &collection.compile;
    let a = crate::config::PWD.clone() + "/rz_build/objects";
    let count = Arc::new(AtomicUsize::new(0));
    let commands = commands.par_iter().map(|x| {
        let count = count.clone();
        let mut state = 0;
        let mut real = String::new();
        let mut object = None;
        for i in x.split_ascii_whitespace() {
            if state == 1 {
                object.replace(i);
                state = 2;
            }
            if i == "-o" {
                state = 1;
            }
            real += " ";
            if i == CONFIG.original_cxx_executable {
                real += &CONFIG.targeted_cxx_executable;
                real += " -emit-llvm ";
            } else if i == CONFIG.original_cc_executable {
                real += &CONFIG.targeted_cc_executable;
                real += " -emit-llvm ";
            } else if state == 2 {
                real += &a;
                real += "/";
                real += percent_encode(i.as_bytes(), crate::FRAGMENT).to_string().as_str();
                state = 0;
            } else {
                real += i;
            }
        }
        trace!("[{}/{}] compiling {}: \n{}", count.fetch_add(1, Ordering::SeqCst), collection.compile.len(), object.as_ref().unwrap(), real);
        (object.map(|x| percent_encode(x.as_bytes(), crate::FRAGMENT).to_string()).unwrap(), real)
    });

    if std::fs::metadata(&a).is_err() {
        std::fs::create_dir(&a).unwrap_or_else(|x| {
            error!("failed to create object dir {:?}: {}", a, x);
            exit(10);
        })
    }
    std::env::set_current_dir(&a).unwrap_or_else(|e|
        {
            error!("cannot change dir: {}", e);
            exit(20);
        }
    );
    commands.for_each(|x| {
        if std::fs::metadata(&x.0).is_ok() {
            info!("found {}, using cached", x.0);
        } else {
            let mut commands = x.1.split_ascii_whitespace();
            std::process::Command::new(commands.next().unwrap())
                .args(commands).spawn()
                .and_then(|mut x| match x.wait() {
                    Ok(e) if e.success() => Ok(()),
                    Ok(e) => Err(std::io::Error::new(ErrorKind::Other, format!("failure with {:?}", e))),
                    Err(e) => Err(e)
                }
                )
                .unwrap_or_else(|e| {
                    error!("cannot compile {}: {:?}", x.0, e)
                });
        }
    });
}