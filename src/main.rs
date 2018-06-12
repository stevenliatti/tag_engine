use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry::{Vacant, Occupied};

extern crate tag_manager;
extern crate walkdir;
use walkdir::WalkDir;

fn main() {
    // mkdir -p a/b/c
    // touch fileA fileB
    let mut tag_to_paths : HashMap<String, HashSet<String>> = HashMap::new();
    let mut path_to_tags : HashMap<String, HashSet<String>> = HashMap::new();

    for entry in WalkDir::new("a").into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().display().to_string();
        println!("{}", path);
        let option = tag_manager::get_tags(&path);
        match option {
            Some(tags) => {
                path_to_tags.insert(path.clone(), tags.clone());
                for tag in tags {
                    match tag_to_paths.entry(tag) {
                        Vacant(entry) => {
                            let mut set = HashSet::new();
                            set.insert(path.clone());
                            entry.insert(set);
                        },
                        Occupied(mut entry) => {
                            entry.get_mut().insert(path.clone());
                        }
                    }
                }
            },
            None => { path_to_tags.insert(path.clone(), HashSet::new()); }
        } 
    }

    println!("tag_to_paths {:#?}", tag_to_paths);
    println!("path_to_tags {:#?}", path_to_tags);
}
