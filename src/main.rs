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
use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use petgraph::dot::{Dot, Config};

extern crate notify;
use notify::{Watcher, RecursiveMode, watcher, DebouncedEvent};
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};
use std::sync::mpsc::channel;
use std::time::Duration;

#[derive(Debug)]
struct Nil;
impl Nil {
    fn new() -> Self { Self {} }
}

#[derive(Debug)]
enum NodeKind {
    Tag,
    File,
    Directory
}

#[derive(Debug)]
struct Node {
    name : String,
    kind : NodeKind
}

impl Node {
    fn new(name : String, kind : NodeKind) -> Self {
        Self { name : name, kind : kind }
    }

    fn set_name(&mut self, name : String) {
        self.name = name;
    }
}

type MyGraph = StableGraph<Node, Nil>;

// TODO: check every call to expect()

fn make_subgraph(root_index : NodeIndex, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, local_path : String, base_path : String) {
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
                    Node::new(String::from(entry), NodeKind::Directory)
                }
                else { Node::new(String::from(entry), NodeKind::File) };
                let new_node = graph.add_node(new_node);
                graph.add_edge(parent_index, new_node, Nil::new());
                update_tags(build_path.clone(), tags_index, graph, new_node);
                parent_index = new_node;
            }
        }
    }
}

fn find_parent(graph : &MyGraph, index : NodeIndex, entry : &str, found : &mut bool) -> NodeIndex {
    for neighbor_index in graph.neighbors(index) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data.kind {
                    NodeKind::File | NodeKind::Directory => {
                        if String::from(entry) == data.name {
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

fn move_entry(root_index : NodeIndex, entry_index : NodeIndex, graph : &mut MyGraph, new_path : String) {
    let mut parent_index = entry_index;
    for neighbor_index in graph.neighbors_directed(entry_index, Direction::Incoming) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data.kind {
                    NodeKind::Directory => {
                        parent_index = neighbor_index;
                        break;
                    },
                    _ => ()
                }
            },
            None => ()
        }
    }
    let new_parent_index = get_node_index(root_index, graph, new_path.clone());
    if parent_index == new_parent_index {
        let mut path_vec : Vec<&str> = new_path.split('/').collect();
        let new_name = path_vec.pop().expect("move_entry, path_vec.pop()").to_string();
        let node = graph.node_weight_mut(entry_index).expect("move_entry, graph.node_weight_mut");
        node.set_name(new_name);
    }
    else {
        let edge = graph.find_edge(parent_index, entry_index);
        match edge {
            Some(edge_index) => { graph.remove_edge(edge_index); },
            None => ()
        }
        graph.add_edge(new_parent_index, entry_index, Nil::new());
    }
}

fn entries_to_remove(entry_index : NodeIndex, graph : &MyGraph,
    entries_index : &mut Vec<NodeIndex>, check_tags_index : &mut Vec<NodeIndex>) {
    entries_index.push(entry_index);
    for neighbor_index in graph.neighbors_directed(entry_index, Direction::Outgoing) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data.kind {
                    NodeKind::Directory =>
                        entries_to_remove(neighbor_index, graph, entries_index, check_tags_index),
                    NodeKind::File => entries_index.push(neighbor_index),
                    NodeKind::Tag => check_tags_index.push(neighbor_index)
                }
            },
            None => ()
        }
    }
}

fn remove_entries(entry_index : NodeIndex, graph : &mut MyGraph, tags_index : &mut HashMap<String, NodeIndex>) {
    let mut entries_index = Vec::new();
    let mut check_tags_index = Vec::new();
    entries_to_remove(entry_index, graph, &mut entries_index, &mut check_tags_index);
    for index in entries_index.into_iter().rev() {
        graph.remove_node(index);
    }
    for tag_index in check_tags_index {
        if graph.edges(tag_index).count() == 0 {
            tags_index.remove(&graph.node_weight(tag_index).unwrap().name);
            graph.remove_node(tag_index);
        }
    }
}

fn get_tags(graph : &MyGraph, tag_index : NodeIndex) -> HashSet<String> {
    let mut tags = HashSet::new();
    for neighbor_index in graph.neighbors_directed(tag_index, Direction::Incoming) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data.kind {
                    NodeKind::Tag => { tags.insert(data.name.clone()); },
                    _ => ()
                }
            },
            None => ()
        }
    }
    tags
}

fn add_tags(tags_to_add : Difference<String, RandomState>, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, entry_index : NodeIndex) {
    for tag in tags_to_add {
        match tags_index.entry(tag.clone()) {
            Vacant(entry) => {
                let new_node_tag = graph.add_node(Node::new(tag.clone(), NodeKind::Tag));
                entry.insert(new_node_tag);
                graph.add_edge(entry_index, new_node_tag, Nil::new());
                graph.add_edge(new_node_tag, entry_index, Nil::new());
            },
            Occupied(entry) => {
                let &tag_index = entry.get();
                    graph.add_edge(entry_index, tag_index, Nil::new());
                    graph.add_edge(tag_index, entry_index, Nil::new());
                }
            }
        }
    }

fn remove_tags(tags_to_remove : Difference<String, RandomState>, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, entry_index : NodeIndex) {
    for tag in tags_to_remove {
        match tags_index.entry(tag.clone()) {
            Occupied(entry) => {
                let &tag_index = entry.get();
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
            },
            Vacant(_) => ()
        }
    }
}

fn update_tags(path : String, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, entry_index : NodeIndex) {
    let existent_tags = get_tags(graph, entry_index);
    let fresh_tags = match tag_manager::get_tags(&path) {
        Some(tags) => tags,
        None => HashSet::new()
    };
    remove_tags(existent_tags.difference(&fresh_tags), tags_index, graph, entry_index);
    add_tags(fresh_tags.difference(&existent_tags), tags_index, graph, entry_index);
}

// fn rename_tag() {}

fn make_graph(path_root : String, base_path : String) -> (MyGraph, HashMap<String, NodeIndex>, NodeIndex) {
    let mut graph : MyGraph = StableGraph::new();
    let mut tags_index = HashMap::new();
    let root_index = graph.add_node(Node::new(path_root.clone(), NodeKind::Directory));
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

fn get_node_index(root_index : NodeIndex, graph : &MyGraph, path : String) -> NodeIndex {
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
    graph : &mut MyGraph, root_index : NodeIndex, base : String) {
    match event {
        Create(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, create, path").to_string();
            let local = local_path(&mut path, base.clone());
            println!("create : {:?}", local);
            make_subgraph(root_index, tags_index, graph, local, base.clone());
        },
        Chmod(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, chmod, path").to_string();
            let local = local_path(&mut path.clone(), base);
            println!("chmod : {:?}", local);
            let entry_index = get_node_index(root_index, graph, local);
            update_tags(path, tags_index, graph, entry_index);
        },
        Remove(path) => {
            let mut path = path.as_path().to_str().expect("dispatcher, remove, path").to_string();
            let local = local_path(&mut path.clone(), base);
            println!("remove : {:?}", local);
            let entry_index = get_node_index(root_index, graph, local);
            remove_entries(entry_index, graph, tags_index);
        },
        Rename(old_path, new_path) => {
            let mut old_path = old_path.as_path().to_str().expect("dispatcher, rename, old_path").to_string();
            let new_path = new_path.as_path().to_str().expect("dispatcher, rename, new_path").to_string();
            let old_local = local_path(&mut old_path.clone(), base.clone());
            let new_local = local_path(&mut new_path.clone(), base.clone());
            println!("rename, old_path : {:?}, new_path : {:?}", old_local, new_local);
            let entry_index = get_node_index(root_index, graph, old_local);
            move_entry(root_index, entry_index, graph, new_local);
        }
        _ => ()
    }
}

fn write_dot_image(graph : &MyGraph, dot_name : &str, image_name : &str) {
    let mut file = File::create(dot_name).expect("file create");
    let graph_dot = format!("{:?}", Dot::with_config(graph, &[Config::EdgeNoLabel]));
    file.write(graph_dot.as_bytes()).expect("file write");
    let mut output = String::from("-o");
    output.push_str(image_name);
    let _exec_dot = Command::new("dot").args(&["-Tjpg", output.as_str(), dot_name]).output().expect("exec");
}

fn main() {
    let absolute_path_root = "/home/stevenliatti/Bureau/a";
    let (base, _) = split_root_path(&mut absolute_path_root.to_string());
    let (mut graph, mut tags_index, root_index) = make_graph(String::from(absolute_path_root), base.clone());

    let dot_name = "graph.dot";
    let image_name = "graph.jpg";
    write_dot_image(&graph, dot_name, image_name);
    
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
                    write_dot_image(&graph, dot_name, image_name);
                // }
                // i = i + 1;
            },
            Err(e) => println!("watch error: {:?}", e)
        }
    }
}
