use std::collections::HashMap;
use std::collections::hash_map::Entry::Vacant;
// use std::fs::FileType;

extern crate tag_manager;
extern crate walkdir;
use walkdir::WalkDir;

extern crate petgraph;
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;

#[derive(Debug)]
enum Node {
    Tag(String),
    File(String),
    Directory(String)
}

fn find_parent(graph : &Graph<Node, String>, index : NodeIndex, entry : &str) -> NodeIndex {
    for neighbor_index in graph.neighbors_directed(index, Direction::Outgoing) {
        match graph.node_weight(neighbor_index) {
            Some(data) => {
                match data {
                    // TODO: maybe no need of or (directory only ?)
                    &Node::File(ref name) | &Node::Directory(ref name) => {
                        println!("name {}", name);
                        if String::from(entry) == name.to_string() {
                            return neighbor_index;
                        }
                        else {
                            return index;
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

fn main() {
    let path_root = "a";
    let mut graph : Graph<Node, String> = Graph::new();
    let root_index = graph.add_node(Node::Directory(String::from(path_root)));
    let mut current_index : NodeIndex;

    let mut tags_index = HashMap::new();

    for entry in WalkDir::new(path_root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path().display().to_string();
        println!("path {}", path);
        
        let mut path_vec : Vec<&str> = path.split('/').collect();
        let new_item = path_vec.pop().unwrap();
        // remove path_root
        if !path_vec.is_empty() {
            path_vec.remove(0);
            current_index = root_index;
            for entry in path_vec {
                current_index = find_parent(&graph, current_index, entry);
            }
            // TODO: check type (file or directory)
            let new_node = graph.add_node(Node::Directory(String::from(new_item)));
            graph.add_edge(current_index, new_node, String::from("yes"));
        }

        // TODO: add tag to graph
        let option = tag_manager::get_tags(&path);
        match option {
            Some(tags) => {
                for tag in tags {
                    match tags_index.entry(tag.clone()) {
                        Vacant(entry) => {
                            let new_node_tag = graph.add_node(Node::Tag(tag.clone()));
                            entry.insert(new_node_tag);
                        },
                        _ => ()
                    }
                }

            },
            None => ()
        }
    }

    println!("tags_index {:#?}", tags_index);
    println!("graph {:#?}", graph);
}
