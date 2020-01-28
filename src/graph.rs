use std::io::{BufRead, ErrorKind};
use std::process::{exit, Stdio};

use log::*;
use percent_encoding::percent_encode;
use rayon::prelude::*;
use serde::*;
use crate::cmaker::Collection;

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphNode {
    name: String,
    uses: usize,
    address: usize,
    call_list: Vec<String>
}

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
                })
                .and_then(|_| process_graph(output.as_str()))
                .unwrap_or_else(|e| {
                error!("failed to gen callgraph for {}: {}", m, e);
            });
        } else {
            info!("found {}, using cached", output);
        }
    });
}

pub fn process_graph(path: &str) -> std::io::Result<()> {
    let file = std::fs::read_to_string(path);

    file.map(|f| {
        let mut sections = Vec::new();
        let mut buffer = Vec::new();
        for l in f.lines() {
            let l = l.trim();
            if l.is_empty() {continue; }
            if l.starts_with("Call graph node") && !buffer.is_empty() {
                sections.push(buffer);
                buffer = Vec::new();
            }
            buffer.push(String::from(l));
        }
        if !buffer.is_empty() {
            sections.push(buffer);
        }
        sections }
    )
        .map(|x| x.iter().map(parse_node).collect::<Vec<_>>())
        .map(|mut x| {
            x.sort_unstable_by(|x, y| x.address.cmp(&y.address));
            x.dedup_by(|x, y |x.address == y.address);
            serde_json::to_string_pretty(&x).unwrap()
        })
        .and_then(|x| std::fs::write(path, x))

}

pub fn parse_node(section: &Vec<String>) -> GraphNode {
    let head = section.first().unwrap();
    let stream = head.split(' ');
    let mut flag = 0;
    let mut u : usize = 0;
    let mut address : usize = 0;
    let mut name : String = String::new();
    for w in stream {
        let w = w.trim();
        if w.is_empty() {continue; }
        if flag == 1 {
            let content : Vec<&str> = w.trim_start_matches('\'').trim_end_matches('>').split("'<<").collect();
            address = usize::from_str_radix(content[1].trim_start_matches("0x"), 16).unwrap();
            name = String::from(content[0]);
        }
        if flag == 2 {
            let content = w.trim_start_matches("#uses=");
            u = content.parse().unwrap();
            break;
        }
        if flag != 0 || w.ends_with(':') {
            flag += 1;
        }
    }
    let k = section[1..]
        .iter()
        .filter(|x|!x.contains("external node"))
        .map(|x| x.split(' ').last().unwrap())
        .map(|x| String::from(x
            .trim_matches('\'')))
        .collect();
    GraphNode {
        name,
        uses: u,
        address,
        call_list: k
    }
}

