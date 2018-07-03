use std::io::prelude::*;
use std::fs::File;
use std::process::Command;
use std::thread;
use std::sync::{Mutex, Arc};
use std::sync::mpsc::channel;
use std::time::Duration;

extern crate petgraph;
use petgraph::dot::{Dot, Config};

extern crate notify;
use notify::{Watcher, RecursiveMode, watcher};
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};

extern crate tag_manager;

extern crate tag_engine;
use tag_engine::MyGraph;

// TODO: check every call to expect()

fn split_root_path(absolute_path : &mut String) -> (String, String) {
    let clone = absolute_path.clone();
    let mut path_vec : Vec<&str> = clone.split('/').collect();
    let local_path = path_vec.pop().expect("split_root, local_path").to_string();
    absolute_path.truncate(clone.len() - local_path.len());
    (absolute_path.clone(), local_path)
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
    let (graph, tags_index, root_index) = tag_engine::make_graph(String::from(absolute_path_root), base.clone());
    println!("graph {:#?}, tags_index {:#?}", graph, tags_index);

    let dot_name = "graph.dot";
    let image_name = "graph.jpg";
    write_dot_image(&graph, dot_name, image_name);
    
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).expect("watcher");
    watcher.watch(absolute_path_root, RecursiveMode::Recursive).expect("watcher watch");

    let graph = Arc::new(Mutex::new(graph));
    let tags_index = Arc::new(Mutex::new(tags_index));
    let main_graph = Arc::clone(&graph);
    let main_tags_index = Arc::clone(&tags_index);

    let base_clone = base.clone();
    thread::spawn(move || {
        tag_engine::socket_server(base_clone, &graph, &tags_index);
    });

    loop {
        match rx.recv() {
            Ok(event) => {
                match event {
                    Create(_) | Chmod(_) | Remove(_) | Rename(_, _) => {
                        let mut ref_graph = main_graph.lock().unwrap();
                        let mut ref_tags_index = main_tags_index.lock().unwrap();
                        tag_engine::dispatcher(event, &mut ref_tags_index, &mut ref_graph, root_index, base.clone());
                        write_dot_image(&ref_graph, dot_name, image_name);
                    }
                    _ => ()
                }
            },
            Err(e) => println!("watch error: {:?}", e)
        }
    }
}
