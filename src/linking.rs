use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::*;
use std::sync::atomic::Ordering::{Relaxed, SeqCst};

use hashbrown::{HashMap, HashSet};
use log::*;
use rayon::prelude::*;

use crate::cmaker::*;

#[derive(Copy, Clone)]
struct Wrapper(*const LinkScript);

unsafe impl Sync for Wrapper {}

unsafe impl Send for Wrapper {}

impl Deref for Wrapper {
    type Target = LinkScript;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

pub(crate) fn linking(c: Arc<Collection>) {
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
            if map.get_mut(i).map(|y| {
                y.1.push(Wrapper(x as _));
            }).is_some() {
                map.get(x.target.abs_path.as_str()).unwrap().0.fetch_add(1, SeqCst);
            };
        }
    });

    let map = Arc::new(map);
    let set = Arc::new(set);
    let finished = Arc::new(AtomicUsize::new(0));
    let q = Arc::new(crossbeam::queue::SegQueue::new());

    c.scripts.par_iter().for_each(|m| {
        let u = map.get(m.target.abs_path.as_str()).unwrap();
        if u.0.load(Relaxed) == 0 {
            q.push(Wrapper(m as _))
        }
    });

    let mut threads = Vec::new();
    for _ in 0..num_cpus::get() {
        let c = c.clone();
        let obj_path = obj_path.clone();
        let map = map.clone();
        let set = set.clone();
        let finished = finished.clone();
        let q = q.clone();
        threads.push(std::thread::spawn(move || {
            while finished.load(Ordering::Relaxed) != c.scripts.len() {
                match q.pop() {
                    Ok(link) => {
                        info!("linking in progress: {}/{}", finished.load(Ordering::Relaxed), c.scripts.len());
                        let a = obj_path.clone() + "/" +
                            percent_encoding::percent_encode(link.target.abs_path.as_bytes(), crate::FRAGMENT).to_string().as_str();
                        if std::fs::metadata(&a).is_ok() {
                            info!("found {}, using cached", a);
                            for i in &map.get(link.target.abs_path.as_str()).unwrap().1 {
                                for j in &map.get(i.target.abs_path.as_str()) {
                                    j.0.fetch_sub(1, Ordering::SeqCst);
                                    if j.0.load(Ordering::SeqCst) == 0 {
                                        q.push(*i)
                                    }
                                }
                            }
                            finished.fetch_add(1, Ordering::SeqCst);
                            info!("linked {}", link.target.abs_path);
                        } else {
                            let mut command = link.target.dependencies.iter()
                                .filter(|x| map.contains_key(x.as_str()) || set.contains(x.as_str()))
                                .map(|x| obj_path.clone() + "/" +
                                    percent_encoding::percent_encode(x.as_bytes(), crate::FRAGMENT).to_string().as_str())
                                .collect::<Vec<_>>();
                            command.sort_unstable();
                            command.dedup_by(|x, y| x == y);
                            command.push(String::from("-o"));
                            command.push(a);
                            match std::process::Command::new(&crate::config::CONFIG.llvm_link_executable)
                                .args(&command)
                                .spawn()
                                .and_then(|mut x| x.wait()) {
                                Ok(e) if e.success() => {
                                    for i in &map.get(link.target.abs_path.as_str()).unwrap().1 {
                                        for j in &map.get(i.target.abs_path.as_str()) {
                                            j.0.fetch_sub(1, Ordering::SeqCst);
                                            if j.0.load(Ordering::SeqCst) == 0 {
                                                q.push(*i)
                                            }
                                        }
                                    }
                                    finished.fetch_add(1, Ordering::SeqCst);
                                    info!("linked {}", link.target.abs_path);
                                }
                                Err(e) => {
                                    error!("failed to link {}: {:?}\ncommand: {} {}", link.target.abs_path, e,
                                           crate::config::CONFIG.llvm_link_executable, command.join(" "));
                                    std::process::exit(50);
                                }
                                Ok(c) => {
                                    error!("failed to link {}: exit with {:?}\ncommand: {} {}", link.target.abs_path, c,
                                           crate::config::CONFIG.llvm_link_executable, command.join(" "));
                                    std::process::exit(50);
                                }
                            }
                        }
                    }
                    _ => spin_loop_hint()
                };
            }
        }));
    }
    for i in threads {
        if i.join().is_err() {
            error!("failed to join linking handle")
        };
    }
}
