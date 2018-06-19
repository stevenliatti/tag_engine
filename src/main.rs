use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_set::Difference;
use std::collections::hash_map::RandomState;
use std::io::prelude::*;
use std::fs::File;
use std::process::Command;

extern crate tag_manager;
extern crate walkdir;
use walkdir::WalkDir;

extern crate petgraph;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use petgraph::dot::{Dot, Config};

extern crate notify;
use notify::{Watcher, RecursiveMode, watcher, DebouncedEvent};
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};
use std::sync::mpsc::channel;
use std::time::Duration;

#[derive(Debug)]
enum Node {
    Tag(String),
    File(String),
    Directory(String)
}

#[derive(Debug)]
struct Nil;
impl Nil {
    fn new() -> Self { Self {} }
}

// TODO: check every call to unwrap()

fn get_node_index(root_index : NodeIndex, graph : &Graph<Node, Nil>, path : String) -> NodeIndex {
    let mut path_vec : Vec<&str> = path.split('/').collect();
    let mut parent_index = root_index;
    if !path_vec.is_empty() {
        // remove path_root
        path_vec.remove(0);
        for entry in path_vec {
            parent_index = find_parent(&graph, parent_index, entry);
        }
    }
    parent_index
}

fn find_parent(graph : &Graph<Node, Nil>, index : NodeIndex, entry : &str) -> NodeIndex {
    for neighbor_index in graph.neighbors(index) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    // TODO: maybe no need of OR (directory only ?)
                    &Node::File(ref name) | &Node::Directory(ref name) => {
                        if String::from(entry) == name.to_string() {
                            return neighbor_index;
                        }
                    },
                    _ => ()
                }
            },
            None => ()
        }
    }
    index
}

fn add_file() {

}

fn add_directory() {

}

fn move_file() {

}

fn move_directory() {

}

fn remove_file() {

}

fn remove_directory() {

}

fn get_tags(graph : &Graph<Node, Nil>, tag_index : NodeIndex) -> HashSet<String> {
    let mut tags = HashSet::new();
    for neighbor_index in graph.neighbors_directed(tag_index, Direction::Incoming) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    &Node::Tag(ref name) => {
                        tags.insert(name.to_string());
                    },
                    _ => ()
                }
            },
            None => ()
        }
    }
    tags
}

fn add_tags(tags_to_add : Difference<String, RandomState>, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, entry_index : NodeIndex) {
    for tag in tags_to_add {
        match tags_index.entry(tag.clone()) {
            Vacant(entry) => {
                let new_node_tag = graph.add_node(Node::Tag(tag.clone()));
                entry.insert(new_node_tag);
                graph.add_edge(entry_index, new_node_tag, Nil::new());
                graph.add_edge(new_node_tag, entry_index, Nil::new());
            },
            Occupied(entry) => {
                let &tag_index = entry.get();
                if !graph.contains_edge(tag_index, entry_index) {
                    graph.add_edge(entry_index, tag_index, Nil::new());
                    graph.add_edge(tag_index, entry_index, Nil::new());
                }
            }
        }
    }
}

fn remove_tags(tags_to_remove : Difference<String, RandomState>, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, entry_index : NodeIndex) {
    for tag in tags_to_remove {
        match tags_index.entry(tag.clone()) {
            Occupied(entry) => {
                let &tag_index = entry.get();
                if graph.contains_edge(tag_index, entry_index) {
                    match graph.find_edge(tag_index, entry_index) {
                        Some(edge) => { graph.remove_edge(edge); },
                        None => ()
                    }
                    match graph.find_edge(entry_index, tag_index) {
                        Some(edge) => { graph.remove_edge(edge); },
                        None => ()
                    }
                }
            },
            Vacant(_) => ()
        }
        if graph.edges(*tags_index.get(tag).unwrap()).count() == 0 {
            let tag_index = tags_index.remove(tag).unwrap();
            graph.remove_node(tag_index);
        }
    }
}

fn update_tags(path : String, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, entry_index : NodeIndex) {
    let existent_tags = get_tags(graph, entry_index);
    let fresh_tags = match tag_manager::get_tags(&path) {
        Some(tags) => tags,
        None => HashSet::new()
    };
    remove_tags(existent_tags.difference(&fresh_tags), tags_index, graph, entry_index);
    add_tags(fresh_tags.difference(&existent_tags), tags_index, graph, entry_index);
}

fn rename_tag() {

}

fn make_graph(path_root : &str) -> (Graph<Node, Nil>, HashMap<String, NodeIndex>, NodeIndex) {
    let mut graph : Graph<Node, Nil> = Graph::new();
    let mut tags_index = HashMap::new();
    let root_index = graph.add_node(Node::Directory(String::from(path_root)));
    update_tags(String::from(path_root), &mut tags_index, &mut graph, root_index);
    let mut is_root = true;

    for entry in WalkDir::new(path_root).into_iter().filter_map(|e| e.ok()) {
        if is_root {
            is_root = false;
            continue;
        }
        let path = entry.path().display().to_string();
        let mut path_vec : Vec<&str> = path.split('/').collect();
        let new_entry = path_vec.pop().unwrap();
        let parent_index = get_node_index(root_index, &graph, path.clone());

        let new_node = if entry.file_type().is_dir() { Node::Directory(String::from(new_entry)) }
        else { Node::File(String::from(new_entry)) };
        
        let new_node = graph.add_node(new_node);
        graph.add_edge(parent_index, new_node, Nil::new());
        update_tags(path.clone(), &mut tags_index, &mut graph, new_node);
    }
    (graph, tags_index, root_index)
}

fn dispatcher(event : DebouncedEvent, tags_index : &mut HashMap<String, NodeIndex>, 
    graph : &mut Graph<Node, Nil>, root_index : NodeIndex) {
    match event {
        Create(path) => println!("create : {:?}", path),
        Chmod(path) => {
            let path = path.as_path().to_str().unwrap().to_string();
            println!("chmod : {:?}", path);
            let entry_index = get_node_index(root_index, graph, path.clone());
            update_tags(path.clone(), tags_index, graph, entry_index);
        },
        Remove(path) => println!("remove : {:?}", path),
        Rename(old_path, new_path) => println!("rename, old_path : {:?}, new_path : {:?}", old_path, new_path),
        _ => ()
    }
}

fn main() {
    let path_root = "a";
    let (mut graph, mut tags_index, root_index) = make_graph(path_root);

    let output_name = "graph.dot";
    let mut file = File::create(output_name).unwrap();
    let graph_dot = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    file.write(graph_dot.as_bytes()).unwrap();
    let _exec_dot = Command::new("dot").args(&["-Tjpg", "-otest.jpg", output_name]).output().unwrap();

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    watcher.watch(path_root, RecursiveMode::Recursive).unwrap();

    loop {
        match rx.recv() {
            Ok(event) => {
                dispatcher(event, &mut tags_index, &mut graph, root_index);
                let mut file = File::create(output_name).unwrap();
                let graph_dot = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
                file.write(graph_dot.as_bytes()).unwrap();
                let _exec_dot = Command::new("dot").args(&["-Tjpg", "-otest.jpg", output_name]).output().unwrap();
            },
            Err(e) => println!("watch error: {:?}", e)
        }
    }
}
