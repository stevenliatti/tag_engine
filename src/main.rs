use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

extern crate tag_manager;
extern crate walkdir;
use walkdir::WalkDir;

extern crate petgraph;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;
use petgraph::dot::{Dot, Config};

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

fn find_parent(graph : &Graph<Node, Nil>, index : NodeIndex, entry : &str) -> NodeIndex {
    for neighbor_index in graph.neighbors_directed(index, Direction::Outgoing) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    // TODO: maybe no need of or (directory only ?)
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

fn check_tag(path : String, tags_index : &mut HashMap<String, NodeIndex>,
    graph : &mut Graph<Node, Nil>, new_node : NodeIndex) {
    let option = tag_manager::get_tags(&path);
    match option {
        Some(tags) => {
            for tag in tags {
                match tags_index.entry(tag.clone()) {
                    Vacant(entry) => {
                        let new_node_tag = graph.add_node(Node::Tag(tag.clone()));
                        entry.insert(new_node_tag);
                        graph.add_edge(new_node, new_node_tag, Nil::new());
                        graph.add_edge(new_node_tag, new_node, Nil::new());
                    },
                    Occupied(entry) => {
                        let &tag_index = entry.get();
                        graph.add_edge(new_node, tag_index, Nil::new());
                        graph.add_edge(tag_index, new_node, Nil::new());
                    }
                }
            }

        },
        None => ()
    }
}

fn make_graph(path_root : &str) -> Graph<Node, Nil> {
    let mut graph : Graph<Node, Nil> = Graph::new();
    let root_index = graph.add_node(Node::Directory(String::from(path_root)));
    let mut current_index : NodeIndex;
    let mut tags_index = HashMap::new();
    check_tag(String::from(path_root), &mut tags_index, &mut graph, root_index);

    for entry in WalkDir::new(path_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().display().to_string();
        let mut path_vec : Vec<&str> = path.split('/').collect();
        let new_item = path_vec.pop().unwrap();
        if !path_vec.is_empty() {
            // remove path_root
            path_vec.remove(0);
            current_index = root_index;
            for entry in path_vec { current_index = find_parent(&graph, current_index, entry); }
            let new_node = if entry.file_type().is_dir() {
                Node::Directory(String::from(new_item))
            }
            else {
                Node::File(String::from(new_item))
            };
            let new_node = graph.add_node(new_node);
            graph.add_edge(current_index, new_node, Nil::new());
            check_tag(path.clone(), &mut tags_index, &mut graph, new_node);
        }
    }

    graph
}

fn main() {
    let path_root = "a";
    let graph = make_graph(path_root);
    println!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
}
