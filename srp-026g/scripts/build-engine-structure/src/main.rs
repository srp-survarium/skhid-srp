//! The idea behind this script is to build a structure out of decompiled function names.
//! It is a pain to parse C functions, so everything is done on the best effort only:
//! * Generics are ignored
//! * Functions returning functions are generally skipped
//! * Operator overloads all stored in a single file
//! * `vector destructor iterator' and similar remove quotes and replace spaces with underscores

use std::collections::HashMap;

pub mod parser;

fn main() {
    let from = "D:/Projects/decompiled_funcs/decompiled_funcs";
    let dir_name = "./resources/ida-funcs";

    let mut dir = Directory::new();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if !entry.metadata().unwrap().is_file() {
            continue;
        }

        let path = entry.path();
        let contents = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            let path = path.to_string_lossy();
            panic!("Failed to read a file to a string: {path}: {error}")
        });

        // Decompilation failed
        if contents == "None" {
            continue;
        }

        let func_name = parser::parse_function_name(&contents);
        dir.insert(&func_name, contents);
    }

    _ = std::fs::create_dir(dir_name);
    dir.write(dir_name.to_string());
}

#[derive(Default, Debug)]
struct Directory {
    dirs: HashMap<String, Self>,
    files: HashMap<String, Vec<String>>,
}

impl Directory {
    fn new() -> Self {
        Self::default()
    }

    fn insert(&mut self, func_name: &str, contents: String) {
        let func_name = match func_name.contains("::") {
            true => func_name.to_string(),
            false => format!("0::{func_name}"),
        };
        let mut iter = func_name.split("::").peekable();

        let mut directory = self;
        while let Some(ident) = iter.next() {
            let is_last = iter.peek().is_none();
            match is_last {
                false => {
                    directory = directory.dirs.entry(ident.to_string()).or_default();
                }
                true => {
                    directory
                        .files
                        .entry(ident.to_string())
                        .or_default()
                        .push(contents);
                    break;
                }
            }
        }
    }

    fn write(self, path: String) {
        for (dir, this) in self.dirs {
            let mut path = path.clone();
            path.push('/');
            path.push_str(&dir);

            _ = std::fs::create_dir(&path);

            this.write(path);
        }

        for (file, contents) in self.files {
            std::fs::write(format!("{path}/{file}.c"), contents.join("\n\n")).unwrap_or_else(
                |error| {
                    println!("{contents:?}");
                    panic!("Couldn't create file: '{path}/{file}.c': '{error:?}'")
                },
            )
        }
    }
}
