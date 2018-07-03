use std::collections::{HashMap, HashSet};
use std::io::prelude::*;
use std::sync::{Mutex, Arc};
use std::os::unix::net::{UnixListener, UnixStream};

extern crate walkdir;

extern crate petgraph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;

extern crate tag_manager;

use graph::{MyGraph, NodeKind};
use parse::{Arg, Operator};
use parse::infix_to_postfix;

#[derive(Debug, Clone)]
enum RequestKind {
    Entries(String),
    Tags,
    RenameTag(String),
    // AddDirectory(String)
}

fn parse_request(stream : &mut UnixStream) -> Option<RequestKind> {
    const BUFFER_SIZE : usize = 4096;
    const CODE_SIZE : usize = 3;
    let mut buffer = [0; BUFFER_SIZE];
    let size = stream.read(&mut buffer).unwrap();
    if size >= CODE_SIZE {
        let mut request = String::new();
        for i in CODE_SIZE..size {
            request.push(buffer[i] as char);
        }
        request = request.trim().to_string();

        let mut kind_string = String::new();
        for i in 0..CODE_SIZE {
            kind_string.push(buffer[i] as char);
        }

        let kind : RequestKind;
        if kind_string == String::from("0x0") {
            kind = RequestKind::Entries(request);
        }
        else if kind_string == String::from("0x1") {
            kind = RequestKind::Tags;
        }
        else if kind_string == String::from("0x2") {
            kind = RequestKind::RenameTag(request);
        }
        else {
            return None;
        }
        return Some(kind);
    }
    None
}

fn make_path_vec(graph : &MyGraph, entry : NodeIndex, path_vec : &mut Vec<String>) {
    path_vec.push(graph.node_weight(entry).unwrap().name.clone());
    for neighbor in graph.neighbors_directed(entry, Direction::Incoming) {
        match graph.node_weight(neighbor).unwrap().kind {
            NodeKind::Directory => {
                make_path_vec(graph, neighbor, path_vec);
            },
            _ => ()
        }
    }
}

fn make_path(graph : &MyGraph, entry : NodeIndex, base_path : String) -> String {
    let mut path_vec = Vec::new();
    make_path_vec(&graph, entry, &mut path_vec);
    let mut path = base_path.clone();
    for entry in path_vec.into_iter().rev() {
        path.push_str(&entry);
        path.push_str("/");
    }
    path.pop();
    path
}

fn entries(graph : &MyGraph, tag_index : NodeIndex, base_path : String) -> Vec<String> {
    let mut nodes_names = Vec::new();
    for entry in graph.neighbors(tag_index) {
        nodes_names.push(make_path(graph, entry, base_path.clone()));
    }
    nodes_names.sort();
    nodes_names
}

fn write_response(entries : Vec<String>, stream : &mut UnixStream) {
    let mut response : Vec<u8> = Vec::new();
    for name in entries {
        for byte in name.as_bytes() {
            response.push(*byte);
        }
        response.push('\n' as u8);
    }
    stream.write(response.as_slice()).unwrap();
    stream.flush().unwrap();
}

fn expression_to_entries(request : String, graph : &MyGraph, tags_index : &HashMap<String, 
    NodeIndex>, base_path : String) -> Vec<String> {
    let postfix = infix_to_postfix(request.clone());
    let mut stack = Vec::new();
    for arg in postfix {
        match arg {
            Arg::Operand(tag) => {
                if tags_index.contains_key(&tag) {
                    let tag_index = tags_index.get(&tag).unwrap();
                    let tags_set : HashSet<NodeIndex> = graph.neighbors(*tag_index).collect();
                    stack.push(tags_set);
                }
                else { stack.push(HashSet::new()); }
            },
            Arg::Operator(op) => {
                if stack.len() >= 2 {
                    let operand_two = stack.pop().unwrap();
                    let operand_one = stack.pop().unwrap();
                    match op {
                        Operator::AND => stack.push(operand_one.intersection(&operand_two).map(|e| *e).collect()),
                        Operator::OR => stack.push(operand_one.union(&operand_two).map(|e| *e).collect())
                    }
                }
            }
        }
    }
    let mut nodes_names = Vec::new();
    if stack.len() == 1 {
        for entry in stack.pop().unwrap() {
            nodes_names.push(make_path(graph, entry, base_path.clone()));
        }
        nodes_names.sort();
    }
    nodes_names
}

fn request_entries(request : String, graph_thread : &Arc<Mutex<MyGraph>>, 
    tags_index_thread : &Arc<Mutex<HashMap<String, NodeIndex>>>, base_path : String, 
    stream : &mut UnixStream) {
    println!("Request for Entries {:?}", request);
    let graph = graph_thread.lock().unwrap();
    let tags_index = tags_index_thread.lock().unwrap();
    let entries = expression_to_entries(request, &graph, &tags_index, base_path);
    if entries.is_empty() {
        stream.write("No files\n".as_bytes()).unwrap();
        stream.flush().unwrap();
    }
    else {
        write_response(entries, stream);
    }
}

fn request_tags(tags_index_thread : &Arc<Mutex<HashMap<String, NodeIndex>>>, stream : &mut UnixStream) {
    println!("Request for Tags");
    let tags_index = tags_index_thread.lock().unwrap();
    let mut entries : Vec<String> = tags_index.keys().map(|key| key.clone()).collect();
    entries.sort();
    write_response(entries, stream);
}

fn request_rename_tag(request : String, graph_thread : &Arc<Mutex<MyGraph>>, 
    tags_index_thread : &Arc<Mutex<HashMap<String, NodeIndex>>>, base_path : String, 
    stream : &mut UnixStream) {
    println!("Request for RenameTag {:?}", request);
    let v : Vec<&str> = request.split(' ').collect();
    if v.len() == 2 {
        let old_name = v[0];
        let new_name = v[1];
        let mut graph = graph_thread.lock().unwrap();
        let mut tags_index = tags_index_thread.lock().unwrap();
        match tags_index.remove(old_name) {
            Some(index) => {
                tags_index.insert(new_name.to_string(), index);
                graph.node_weight_mut(index).unwrap().name = new_name.to_string();
                let mut entries = entries(&graph, index, base_path.clone());
                for e in &entries {
                    tag_manager::rename_tag(e, old_name.to_string(), new_name.to_string());
                }
                entries.insert(0, format!("Rename {:?} to {:?} for files :", old_name, new_name));
                write_response(entries, stream);
            },
            None => {
                write_response(vec![String::from("No tag with this old name")], stream);
            }
        }
    }
    else {
        write_response(vec![String::from("Bad request")], stream);
    }
}

pub fn server(base_path : String, graph : &Arc<Mutex<MyGraph>>, tags_index : &Arc<Mutex<HashMap<String, NodeIndex>>>) {
    let listener = UnixListener::bind("/tmp/tag_engine").unwrap();
    let graph_thread = Arc::clone(graph);
    let tags_index_thread = Arc::clone(tags_index);

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        match parse_request(&mut stream) {
            Some(kind) => match kind {
                RequestKind::Entries(request) => request_entries(request, &graph_thread, 
                    &tags_index_thread, base_path.clone(), &mut stream),
                RequestKind::Tags => request_tags(&tags_index_thread, &mut stream),
                RequestKind::RenameTag(request) => request_rename_tag(request, &graph_thread, 
                    &tags_index_thread, base_path.clone(), &mut stream)
            },
            None => {
                stream.write("Invalid request\n".as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        }
    }
}
