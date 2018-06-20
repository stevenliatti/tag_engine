use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_set::Difference;
use std::collections::hash_map::RandomState;
use std::io::prelude::*;
use std::fs::File;
use std::process::Command;
use std::fs::metadata;

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

// TODO: check every call to expect()

fn make_subgraph(root_index : NodeIndex, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, local_path : String, base_path : String) {
    let mut path_vec : Vec<&str> = local_path.split('/').collect();
    let mut parent_index = root_index;
    let mut found = false;
    let mut build_path : String = base_path;
    build_path.push_str(path_vec[0]);
    if !path_vec.is_empty() {
        // remove path_root
        path_vec.remove(0);
        for entry in path_vec {
            build_path.push('/');
            build_path.push_str(entry);
            parent_index = find_parent(&graph, parent_index, entry, &mut found);
            if !found {
                let new_node = if metadata(build_path.clone())
                    .expect("make_subgraph, new_node, metadata").file_type().is_dir() {
                    Node::Directory(String::from(entry))
                }
                else { Node::File(String::from(entry)) };
                let new_node = graph.add_node(new_node);
                graph.add_edge(parent_index, new_node, Nil::new());
                update_tags(build_path.clone(), tags_index, graph, new_node);
                parent_index = new_node;
            }
        }
    }
}

fn find_parent(graph : &Graph<Node, Nil>, index : NodeIndex, entry : &str, found : &mut bool) -> NodeIndex {
    for neighbor_index in graph.neighbors(index) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    &Node::File(ref name) | &Node::Directory(ref name) => {
                        if String::from(entry) == name.to_string() {
                            *found = true;
                            return neighbor_index;
                        }
                    },
                    _ => ()
                }
            },
            None => ()
        }
    }
    *found = false;
    index
}

// fn move_file() {}
// fn move_directory() {}
// fn remove_file() {}
// fn remove_directory() {}

fn get_tags(graph : &Graph<Node, Nil>, tag_index : NodeIndex) -> HashSet<String> {
    let mut tags = HashSet::new();
    for neighbor_index in graph.neighbors_directed(tag_index, Direction::Incoming) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    &Node::Tag(ref name) => { tags.insert(name.to_string()); },
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
                    if graph.edges(tag_index).count() == 0 {
                        entry.remove();
                        graph.remove_node(tag_index);
                    }
                }
            },
            Vacant(_) => ()
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

// fn rename_tag() {}

fn make_graph(path_root : String, base_path : String) -> (Graph<Node, Nil>, HashMap<String, NodeIndex>, NodeIndex) {
    let mut graph : Graph<Node, Nil> = Graph::new();
    let mut tags_index = HashMap::new();
    let root_index = graph.add_node(Node::Directory(path_root.clone()));
    update_tags(path_root.clone(), &mut tags_index, &mut graph, root_index);
    let mut is_root = true;

    for entry in WalkDir::new(path_root).into_iter().filter_map(|e| e.ok()) {
        if is_root {
            is_root = false;
            continue;
        }
        let path = local_path(&mut entry.path().display().to_string(), base_path.clone());
        make_subgraph(root_index, &mut tags_index, &mut graph, path, base_path.clone());
    }
    (graph, tags_index, root_index)
}

fn split_root_path(absolute_path : &mut String) -> (String, String) {
    let clone = absolute_path.clone();
    let mut path_vec : Vec<&str> = clone.split('/').collect();
    let local_path = path_vec.pop().expect("split_root, local_path").to_string();
    absolute_path.truncate(clone.len() - local_path.len());
    (absolute_path.clone(), local_path)
}

fn local_path(absolute_path : &mut String, base_path : String) -> String {
    absolute_path.split_off(base_path.len())
}

fn get_node_index(root_index : NodeIndex, graph : &Graph<Node, Nil>, path : String) -> NodeIndex {
    let mut path_vec : Vec<&str> = path.split('/').collect();
    let mut parent_index = root_index;
    let mut found = false;
    if !path_vec.is_empty() {
        // remove path_root
        path_vec.remove(0);
        for entry in path_vec {
            parent_index = find_parent(&graph, parent_index, entry, &mut found);
        }
    }
    parent_index
}

fn dispatcher(event : DebouncedEvent, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, root_index : NodeIndex, base : String) {
    match event {
        Create(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, create, local").to_string();
            let local = local_path(&mut path, base.clone());
            println!("create : {:?}", local);
            make_subgraph(root_index, tags_index, graph, local, base.clone());
        },
        Chmod(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, chmod, local").to_string();
            let local = local_path(&mut path.clone(), base);
            println!("chmod : {:?}", local);
            let entry_index = get_node_index(root_index, graph, local);
            update_tags(path, tags_index, graph, entry_index);
        },
        Remove(path) => println!("remove : {:?}", path),
        Rename(old_path, new_path) => println!("rename, old_path : {:?}, new_path : {:?}", old_path, new_path),
        _ => ()
    }
}

fn main() {
    let absolute_path_root = "/home/stevenliatti/Bureau/a";
    let (base, local) = split_root_path(&mut absolute_path_root.to_string());
    println!("base {:?}, local {:?}", base, local);
    let (mut graph, mut tags_index, root_index) = make_graph(String::from(absolute_path_root), base.clone());

    let output_name = "graph.dot";
    let mut file = File::create(output_name).expect("file create");
    let graph_dot = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    file.write(graph_dot.as_bytes()).expect("file write");
    let _exec_dot = Command::new("dot").args(&["-Tjpg", "-otest.jpg", output_name]).output().expect("exec");

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).expect("watcher");
    watcher.watch(absolute_path_root, RecursiveMode::Recursive).expect("watcher watch");

    // let mut i = 0;
    loop {
        match rx.recv() {
            Ok(event) => {
                dispatcher(event, &mut tags_index, &mut graph, root_index, base.clone());
                // if _i == 100 {
                    // println!("write !");
                    // i = 0;
                    let mut file = File::create(output_name).expect("file create");
                    let graph_dot = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
                    file.write(graph_dot.as_bytes()).expect("file write");
                    let _exec_dot = Command::new("dot").args(&["-Tjpg", "-otest.jpg", output_name]).output().expect("exec");
                // }
                // i = i + 1;
            },
            Err(e) => println!("watch error: {:?}", e)
        }
    }
}
