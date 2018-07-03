use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::hash_set::Difference;
use std::collections::hash_map::RandomState;
use std::io::prelude::*;
use std::fs::metadata;
use std::sync::{Mutex, Arc};
use std::os::unix::net::{UnixListener, UnixStream};

extern crate walkdir;
use walkdir::WalkDir;

extern crate petgraph;
use petgraph::stable_graph::StableGraph;
use petgraph::graph::NodeIndex;
use petgraph::Direction;

extern crate notify;
use notify::DebouncedEvent;
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};

extern crate tag_manager;

#[derive(Debug, Clone)]
pub struct Nil;
impl Nil {
    fn new() -> Self { Self {} }
}

#[derive(Debug, Clone)]
enum NodeKind {
    Tag,
    File,
    Directory
}

#[derive(Debug, Clone)]
pub struct Node {
    name : String,
    kind : NodeKind
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

// TODO: bug with orphaned tags
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

// TODO: if create dir by mv, scan subgraph
pub fn dispatcher(event : DebouncedEvent, tags_index : &mut HashMap<String, NodeIndex>,
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

fn make_path(graph : &MyGraph, entry : NodeIndex, path_vec : &mut Vec<String>) {
    path_vec.push(graph.node_weight(entry).unwrap().name.clone());
    for neighbor in graph.neighbors_directed(entry, Direction::Incoming) {
        match graph.node_weight(neighbor).unwrap().kind {
            NodeKind::Directory => {
                make_path(graph, neighbor, path_vec);
            },
            _ => ()
        }
    }
}

fn socket_entries(graph : &MyGraph, tag_index : NodeIndex, base_path : String) -> Vec<String> {
    let mut nodes_names = Vec::new();
    for entry in graph.neighbors(tag_index) {
        let mut path_vec = Vec::new();
        make_path(&graph, entry, &mut path_vec);
        let mut path = base_path.clone();
        for entry in path_vec.into_iter().rev() {
            path.push_str(&entry);
            path.push_str("/");
        }
        path.pop();
        nodes_names.push(path);
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

const AND_OPERATOR_STR : &str = "AND";
const OR_OPERATOR_STR : &str = "OR";

#[derive(Debug, Clone)]
enum Operator { AND, OR }

use Operator::*;
impl Operator {
    fn compare(&self, other : &Operator) -> i8 {
        match (self, other) {
            (&AND, &OR) => 1,
            (&OR, &AND) => -1,
            _ => 0
        }
    }

    fn str_to_operator(op_str : &str) -> Option<Self> {
        if op_str == AND_OPERATOR_STR {
            Some(AND)
        }
        else if op_str == OR_OPERATOR_STR {
            Some(OR)
        }
        else {
            None
        }
    }

    fn operator_to_str(op : &Operator) -> String {
        match op {
            &AND => AND_OPERATOR_STR.to_string(),
            &OR => OR_OPERATOR_STR.to_string()
        }
    }
}

fn infix_to_postfix(infix : String) -> Vec<String> {
    let infix : Vec<&str> = infix.split(' ').collect();
    let mut stack = Vec::new();
    let mut postfix = Vec::new();
    for arg in infix {
        if arg == AND_OPERATOR_STR || arg == OR_OPERATOR_STR {
            let arg = Operator::str_to_operator(arg).unwrap();
            if stack.is_empty() {
                stack.push(arg);
            }
            else {
                while !stack.is_empty() {
                    let mut top_stack = stack.get(stack.len() - 1).unwrap().clone();
                    let mut compare = arg.compare(&top_stack);
                    if compare > 0 {
                        break;
                    }
                    else {
                        postfix.push(Operator::operator_to_str(&stack.pop().unwrap()));
                    }
                }
                stack.push(arg);
            }
        }
        else {
            postfix.push(arg.to_string());
        }
    }
    postfix.into_iter().map(|e| e.to_string()).collect()
}

pub fn socket_server(base_path : String, graph : &Arc<Mutex<MyGraph>>, tags_index : &Arc<Mutex<HashMap<String, NodeIndex>>>) {
    let listener = UnixListener::bind("/tmp/tag_engine").unwrap();
    let graph_thread = Arc::clone(graph);
    let tags_index_thread = Arc::clone(tags_index);

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        match parse_request(&mut stream) {
            Some(kind) => match kind {
                RequestKind::Entries(request) => {
                    println!("Request for Entries {:?}", request);
                    let graph = graph_thread.lock().unwrap();
                    let tags_index = tags_index_thread.lock().unwrap();
                    // TODO:
                    let postfix = infix_to_postfix(request.clone());
                    println!("NodeIndex {:?}", tags_index.get(&request));
                    match tags_index.get(&request) {
                        Some(index) => {
                            let entries = socket_entries(&graph, *index, base_path.clone());
                            write_response(entries, &mut stream);
                        },
                        None => {
                            stream.write("No files\n".as_bytes()).unwrap();
                            stream.flush().unwrap();
                        }
                    }
                },
                RequestKind::Tags => {
                    println!("Request for Tags");
                    let tags_index = tags_index_thread.lock().unwrap();
                    let mut entries : Vec<String> = tags_index.keys().map(|key| key.clone()).collect();
                    entries.sort();
                    write_response(entries, &mut stream);
                },
                RequestKind::RenameTag(request) => {
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
                                let mut entries = socket_entries(&graph, index, base_path.clone());
                                for e in &entries {
                                    tag_manager::rename_tag(e, old_name.to_string(), new_name.to_string());
                                }
                                entries.insert(0, format!("Rename {:?} to {:?} for files :", old_name, new_name));
                                write_response(entries, &mut stream);
                            },
                            None => {
                                write_response(vec![String::from("No tag with this old name")], &mut stream);
                            }
                        }
                    }
                    else {
                        write_response(vec![String::from("Bad request")], &mut stream);
                    }
                }
            },
            None => {
                stream.write("Invalid request\n".as_bytes()).unwrap();
                stream.flush().unwrap();
            }
        }
    }
}