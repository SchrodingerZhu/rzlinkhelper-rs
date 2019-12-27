use std::sync::Arc;
use std::sync::atomic::*;

use hashbrown::{HashMap, HashSet};
use log::*;
use rayon::prelude::*;

use crate::cmaker::*;

pub(crate) fn linking(c: &Collection) {
    let mut map = HashMap::new();
    let mut set = HashSet::new();
    let obj_path = format!("{}/rz_build/objects", *crate::config::PWD);
    for i in &c.objects {
        set.insert(i.abs_path.clone());
    }
    for i in &c.scripts {
        map.insert(i.target.abs_path.clone(), (AtomicUsize::new(0), Vec::new()));
    }

    c.scripts.iter().for_each(|x| {
        for i in &x.target.dependencies {
            map.get_mut(i).map(|y| {
                y.0.fetch_add(1, Ordering::SeqCst);
                y.1.push(x.target.abs_path.clone());
            }).unwrap_or(());
        }
    });

    let map = Arc::new(map);
    let set = Arc::new(set);
    let finished = Arc::new(AtomicUsize::new(0));
    let mut q = Vec::new();
    while finished.load(Ordering::Relaxed) != c.scripts.len() {
        info!("linking in progress: {}/{}", finished.load(Ordering::Relaxed), c.scripts.len());
        q.clear();
        for i in &c.scripts {
            if map.get(&i.target.abs_path).unwrap().0.load(Ordering::Relaxed) == 0 {
                q.push(i);
            }
        }
        q.par_iter().for_each(|x| {
            let map = map.clone();
            let set = set.clone();
            let finished = finished.clone();
            let a = obj_path.clone() + "/" +
                percent_encoding::percent_encode(x.target.abs_path.as_bytes(), crate::FRAGMENT).to_string().as_str();
            if std::fs::metadata(&a).is_ok() {
                info!("found {}, using cached", a);
                for i in &map.get(x.target.abs_path.as_str()).unwrap().1 {
                    for j in &map.get(i.as_str()) {
                        j.0.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                finished.fetch_add(1, Ordering::SeqCst);
                info!("linked {}", x.target.abs_path);
            } else {
                let mut command = x.target.dependencies.iter()
                    .filter(|x| map.contains_key(x.as_str()) || set.contains(x.as_str()))
                    .map(|x| obj_path.clone() + "/" +
                        percent_encoding::percent_encode(x.as_bytes(), crate::FRAGMENT).to_string().as_str())
                    .collect::<Vec<_>>();
                command.push(String::from("-o"));
                command.push(a);
                match std::process::Command::new(&crate::config::CONFIG.llvm_link_executable)
                    .args(command)
                    .spawn()
                    .and_then(|mut x| x.wait()) {
                    Ok(e) if e.success() => {
                        for i in &map.get(x.target.abs_path.as_str()).unwrap().1 {
                            for j in &map.get(i.as_str()) {
                                j.0.fetch_sub(1, Ordering::SeqCst);
                            }
                        }
                        finished.fetch_add(1, Ordering::SeqCst);
                        info!("linked {}", x.target.abs_path);
                    }
                    Err(e) => {
                        error!("failed to link {}: {:#?}", x.target.abs_path, e);
                        std::process::exit(50);
                    }
                    Ok(c) => {
                        error!("failed to link {}: exit with {:#?}", x.target.abs_path, c);
                        std::process::exit(50);
                    }
                }
            }
        })
    }
}