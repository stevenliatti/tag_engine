use std::collections::HashMap;

extern crate walkdir;

extern crate petgraph;
use petgraph::graph::NodeIndex;

extern crate notify;
use notify::DebouncedEvent;
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};

extern crate tag_manager;

pub mod graph;
use graph::{MyGraph, local_path, make_subgraph, get_node_index, update_tags, move_entry, remove_entries};

pub mod server;
pub mod parse;

pub fn dispatcher(event : DebouncedEvent, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, root_index : NodeIndex, base : String) {
    match event {
        Create(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, create, path").to_string();
            let local = local_path(&mut path, base.clone());
            println!("========== CREATE  : {:?} ==========", local);
            make_subgraph(root_index, tags_index, graph, local, base.clone());
        },
        Chmod(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, chmod, path").to_string();
            let local = local_path(&mut path.clone(), base);
            println!("========== CHMOD : {:?} ==========", local);
            let entry_index = get_node_index(root_index, graph, local);
            update_tags(path, tags_index, graph, entry_index);
        },
        Remove(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, remove, path").to_string();
            let local = local_path(&mut path.clone(), base);
            println!("========== REMOVE : {:?} ==========", local);
            let entry_index = get_node_index(root_index, graph, local);
            remove_entries(entry_index, graph, tags_index);
        },
        Rename(old_path, new_path) => {
            let mut old_path = old_path.as_path().to_str()
                .expect("dispatcher, rename, old_path").to_string();
            let new_path = new_path.as_path().to_str()
                .expect("dispatcher, rename, new_path").to_string();
            let old_local = local_path(&mut old_path.clone(), base.clone());
            let new_local = local_path(&mut new_path.clone(), base.clone());
            println!("========== RENAME, old_path : {:?}, new_path : {:?} ==========", old_local, new_local);
            let entry_index = get_node_index(root_index, graph, old_local);
            move_entry(root_index, entry_index, graph, new_local);
        }
        _ => ()
    }
}
