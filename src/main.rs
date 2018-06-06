use std::io;
use std::fs;
use std::path::Path;

extern crate tag_manager;

fn visit_dirs(dir: &Path, level: u32) -> io::Result<()> {
    let next_level = level + 1;
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            for _ in 0..level {
                print!("    ");
            }
            if path.is_dir() {
                println!("dir {:?}", path);
                visit_dirs(&path, next_level)?;
            }
            else {
                println!("file {:?}", path);
            }
            let option = tag_manager::get_tags(dir.to_str().unwrap());
            match option {
                Some(tags) => {
                    for _ in 0..level {
                        print!("    ");
                    }
                    println!("Tag(s) {:?} for file {:?}", tags, path);
                },
                None => ()
            }
        }
    }
    Ok(())
}

fn main() {
    // mkdir -p a/b/c
    // touch fileA fileB
    let path = Path::new("a");

    match visit_dirs(path, 0) {
        Ok(_) => (),
        Err(err) => println!("error : {}", err)
    }
}
