use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_set::Difference;
use std::collections::hash_map::RandomState;
use std::fs::metadata;

use walkdir::WalkDir;

use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;

extern crate tag_manager;

#[derive(Debug, Clone)]
pub struct Nil;
impl Nil {
    fn new() -> Self { Self {} }
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Tag,
    File,
    Directory
}

#[derive(Debug, Clone)]
pub struct Node {
    pub name : String,
    pub kind : NodeKind
}

impl Node {
    fn new(name : String, kind : NodeKind) -> Self {
        Self { name, kind }
    }

    fn set_name(&mut self, name : String) {
        self.name = name;
    }
}

pub type MyGraph = StableGraph<Node, Nil>;

// TODO: check every call to expect()

pub fn make_subgraph(root_index : NodeIndex, tags_index : &mut HashMap<String, NodeIndex>,
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

pub fn make_graph(path_root : String, base_path : String) -> (MyGraph, HashMap<String, NodeIndex>, NodeIndex) {
    let mut graph : MyGraph = StableGraph::new();
    let mut tags_index = HashMap::new();
    let local_root = local_path(&mut path_root.clone(), base_path.clone());
    let root_index = graph.add_node(Node::new(local_root, NodeKind::Directory));
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

pub fn local_path(absolute_path : &mut String, base_path : String) -> String {
    absolute_path.split_off(base_path.len())
}

pub fn get_node_index(root_index : NodeIndex, graph : &MyGraph, path : String) -> NodeIndex {
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

pub fn move_entry(root_index : NodeIndex, entry_index : NodeIndex, graph : &mut MyGraph, new_path : String) {
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

// TODO: bug with orphaned tags
pub fn remove_entries(entry_index : NodeIndex, graph : &mut MyGraph, tags_index : &mut HashMap<String, NodeIndex>) {
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

// ----------------------------------------- TAGS -----------------------------------------

pub fn update_tags(path : String, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut MyGraph, entry_index : NodeIndex) {
    let existent_tags = get_tags(graph, entry_index);
    let fresh_tags = match tag_manager::get_tags(&path) {
        Some(tags) => tags,
        None => HashSet::new()
    };
    remove_tags(existent_tags.difference(&fresh_tags), tags_index, graph, entry_index);
    add_tags(fresh_tags.difference(&existent_tags), tags_index, graph, entry_index);
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
                graph.add_edge(new_node_tag, entry_index, Nil::new());
            },
            Occupied(entry) => {
                let &tag_index = entry.get();
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
                    if graph.edges(tag_index).count() == 0 {
                        entry.remove();
                        graph.remove_node(tag_index);
                    }
            },
            Vacant(_) => ()
        }
    }
}
