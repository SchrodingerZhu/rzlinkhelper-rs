use std::io::{Error, ErrorKind, Read, Write};
use std::process::{Command, exit};
use std::process::Stdio;
use serde::*;
use log::*;

use crate::config::CONFIG;

pub fn run_cmake() {
    let compiling = std::fs::create_dir("rz_build")
        .and_then(|_| std::env::set_current_dir("./rz_build"))
        .and_then(|_| Command::new(&CONFIG.cmake_executable)
            .arg("..").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn())
        .and_then(|mut child| {
            let res = child.wait();
            let mut out = String::new();
            let mut err = String::new();
            child.stdout.as_mut().unwrap().read_to_string(&mut out)
                .map(|_| ())
                .unwrap_or_else(|x| warn!("cannot read cmake output {}", x));
            child.stderr.as_mut().unwrap().read_to_string(&mut err)
                .map(|_| ())
                .unwrap_or_else(|x| warn!("cannot read cmake stderr {}", x));
            trace!("cmake output: \n{}", out);
            if !err.is_empty() { warn!("cmake stderr: \n{}", err); }
            match res {
                Ok(e) if e.success() => Ok(()),
                Ok(f) => Err(Error::new(ErrorKind::Other, format!("cmake exit with failure {:?}", f))),
                Err(e) => Err(e)
            }
        });
    if compiling.is_err() {
        error!("failed to run cmake command, {:#?}", compiling.err().unwrap());
        exit(3);
    }
}

pub fn run_remake() {
    let cpu = num_cpus::get();
    info!("start building with {} thread(s). ", cpu);
    let making =
        Command::new(&CONFIG.remake_executable).arg(format!("-j{}", cpu))
            .arg("-x").arg("-Oline").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            .and_then(|mut child| {
                let mut out = String::new();
                let mut err = String::new();
                child.stdout.as_mut().unwrap().read_to_string(&mut out)
                    .map(|_| ())
                    .unwrap_or_else(|x| warn!("cannot read remake output {}", x));
                child.stderr.as_mut().unwrap().read_to_string(&mut err)
                    .map(|_| ())
                    .unwrap_or_else(|x| warn!("cannot read remake stderr {}", x));
                if !err.is_empty() { warn!("remake stderr: \n{}", err); }
                match child.wait() {
                    Ok(e) if e.success() => Ok(out),
                    Ok(f) => Err(Error::new(ErrorKind::Other, format!("remake exit with failure {:?}", f))),
                    Err(e) => Err(e)
                }
            });
    match making {
        Err(e) => {
            error!("failed to run remake command, {:#?}", e);
            exit(3);
        }
        Ok(res) => {
            let storing = std::fs::File::create("remake.log")
                .and_then(|mut f| f.write_all(res.as_bytes()));
            if storing.is_err() {
                error!("failed to store remake log, {:#?}", storing.err().unwrap());
                exit(4);
            }
            info!("remake log saved at {:?}", std::fs::canonicalize("remake.log").unwrap())
        }
    }
}

pub fn run_cmaker() {
    let path_tuple = std::env::current_dir()
        .and_then(|x|
            x.to_str().ok_or(std::io::Error::new(ErrorKind::Other, "cannot initialize path"))
                .map(String::from))
        .and_then(|x|
            std::fs::canonicalize("remake.log")
                .map_err(|e| std::io::Error::new(ErrorKind::Other, e))
                .and_then(|x| x.to_str().ok_or(std::io::Error::new(ErrorKind::Other, "cannot initialize path"))
                    .map(String::from))
                .map(move |y| (x.clone(), y, x + "/cmaker.log"))
        );

    let parsing =
        path_tuple.and_then(|(work, log, output)|
            Command::new(&CONFIG.cmaker_executable)
                .args(&["-w", &work, "-o", &output, "-t", &log])
                .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
                .and_then(|mut child| {
                    let res = child.wait();
                    let mut out = String::new();
                    let mut err = String::new();
                    child.stdout.as_mut().unwrap().read_to_string(&mut out)
                        .map(|_| ())
                        .unwrap_or_else(|x| warn!("cannot read cmaker output {}", x));
                    child.stderr.as_mut().unwrap().read_to_string(&mut err)
                        .map(|_| ())
                        .unwrap_or_else(|x| warn!("cannot read cmaker output {}", x));
                    if !err.is_empty() { warn!("cmaker stderr: \n{}", err); }
                    match res {
                        Ok(e) if e.success() => Ok(out),
                        Ok(f) => Err(Error::new(ErrorKind::Other, format!("cmaker exit with failure {:?}", f))),
                        Err(e) => Err(e)
                    }
                }));
    if parsing.is_err() {
        error!("failed to run cmaker command, {:#?}", parsing.err().unwrap());
        exit(3);
    } else {
        info!("remake log saved at {:?}", std::fs::canonicalize("cmaker.log").unwrap())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Collection {
    pub objects: Vec<Object>,
    pub scripts: Vec<LinkScript>,
    pub compile: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkScript {
    pub abs_path: String,
    pub target: Target,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Object {
    pub abs_path: String,
    pub name: String,
    pub defined_symbols: Vec<Symbol>,
    pub undefined_symbols: Vec<Symbol>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Symbol {
    name: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    pub abs_path: String,
    pub dependencies: Vec<String>,
    // will changed later
    pub target_type: u8,
}


pub fn get_collection(build_dir: &str) -> Collection {
    std::env::set_current_dir(build_dir).unwrap_or_else(|e|
        {
            error!("failed to change dir to ./rz_build, remove .progress if you need: {}", e);
            exit(6);
        }
    );
    std::fs::read_to_string("cmaker.log")
        .and_then(|mut x| simd_json::serde::from_str(&mut x)
            .map_err(|x| std::io::Error::new(ErrorKind::Other, x)))
        .unwrap_or_else(|x| {
            error!("failed to read cmaker.log: {}", x);
            exit(6);
        })
}
