use std::io::prelude::*;
use std::fs::File;
use std::process::Command;
use std::thread;
use std::sync::{Mutex, Arc};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

extern crate petgraph;
use petgraph::dot::{Dot, Config};

extern crate notify;
use notify::{Watcher, RecursiveMode, watcher};
use notify::DebouncedEvent::{Create, Chmod, Remove, Rename};

extern crate tag_manager;

extern crate tag_engine;
use tag_engine::graph::MyGraph;

use std::path::Path;
use std::process::exit;

extern crate clap;
use clap::{App, Arg};

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
    let _exec_dot = Command::new("dot").args(&["-Tpng", output.as_str(), dot_name]).output().expect("exec");
}

fn main() {
    let matches = App::new("Tag Engine").version("0.1.0").author("Steven Liatti")
        .arg(Arg::with_name("path")
            .takes_value(true).required(true).multiple(false))
        .arg(Arg::with_name("debug")
            .short("-d").long("--debug").required(false).multiple(false))
        .get_matches();

    let absolute_path_root = matches.value_of("path").unwrap();
    let path = Path::new(absolute_path_root);
    if !path.exists()  {
        eprintln!("The path doesn't exist");
        exit(1);
    }
    if path.is_relative()  {
        eprintln!("The path must be absolute");
        exit(1);
    }
    if !path.is_dir()  {
        eprintln!("The path must point to a directory");
        exit(1);
    }

    let (base_path, _) = split_root_path(&mut absolute_path_root.to_string());
    let now = Instant::now();
    let (graph, tags_index, root_index) = tag_engine::graph::make_graph(String::from(absolute_path_root), base_path.clone());
    let new_now = Instant::now();
    let elapsed = new_now.duration_since(now);

    let dot_name = "graph.dot";
    let image_name = "graph.png";
    let debug = matches.is_present("debug");
    if debug {
        println!("{}", elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9);
        println!("graph {:#?}, tags_index {:#?}", graph, tags_index);
        write_dot_image(&graph, dot_name, image_name);
    }

    let graph = Arc::new(Mutex::new(graph));
    let tags_index = Arc::new(Mutex::new(tags_index));
    let main_graph = Arc::clone(&graph);
    let main_tags_index = Arc::clone(&tags_index);

    let base_clone = base_path.clone();
    thread::spawn(move || {
        tag_engine::server::server(base_clone, &graph, &tags_index);
    });
    
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).expect("watcher");
    watcher.watch(absolute_path_root, RecursiveMode::Recursive).expect("watcher watch");

    loop {
        match rx.recv() {
            Ok(event) => {
                match event {
                    Create(_) | Chmod(_) | Remove(_) | Rename(_, _) => {
                        let mut ref_graph = main_graph.lock().unwrap();
                        let mut ref_tags_index = main_tags_index.lock().unwrap();
                        tag_engine::dispatcher(event, &mut ref_tags_index, &mut ref_graph, root_index, base_path.clone());
                        if debug {
                            println!("graph {:#?}, tags_index {:#?}", *ref_graph, *ref_tags_index);
                            write_dot_image(&ref_graph, dot_name, image_name);
                        }
                    }
                    _ => ()
                }
            },
            Err(e) => println!("watch error: {:?}", e)
        }
    }
}
