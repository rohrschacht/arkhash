extern crate chrono;

use std::fs::{self, OpenOptions};
use std::path::{PathBuf};
use std::io::{Read, Error, BufReader, Write};
use std::process::Command;
use std::thread;

use self::chrono::DateTime;


pub fn update_directories(opts: super::util::Options) {
    match opts.subdir_mode {
        false => update_hashsums(PathBuf::from("."), opts),
        true => {
            match opts.num_threads {
                0 => {
                    let dir_entries = fs::read_dir(".").unwrap();
                    let mut thread_handles = Vec::new();

                    for entry in dir_entries {
                        let entry = entry.unwrap();
                        let metadata = entry.metadata().unwrap();


                        if metadata.is_dir() {
                            if opts.loglevel_info() {
                                let now: DateTime<chrono::Local> = chrono::Local::now();
                                println!("[{}] Updating Directory {}", now, entry.path().to_str().unwrap());
                            }

                            let thread_path = entry.path().clone();
                            let thread_opts = opts.clone();
                            let handle = thread::spawn(|| {
                                update_hashsums(thread_path, thread_opts);
                            });
                            thread_handles.push(handle);
                        }
                    }

                    for handle in thread_handles {
                        handle.join().unwrap();
                    }
                },
                _ => {

                }
            }
        }
    }
}

fn update_hashsums(path: PathBuf, opts: super::util::Options) {
    let dirwalker = DirWalker::new(&path, opts.subdir_mode);
    let reader = BufReader::new(dirwalker);

    let filter = super::filter::Filter::new(reader, path.to_str().unwrap(), &opts);

    match filter {
        Err(e) => panic!(e),
        Ok(filter) => {
            let mut filepath = path.clone();
            filepath.push(format!("{}sum.txt", opts.algorithm));
            let mut file = OpenOptions::new().create(true).append(true).open(filepath).unwrap();

            for line in filter {
                let hashline = calculate_hash(line, &path, &opts);

                if let Err(e) = write!(file, "{}", hashline) {
                    eprintln!("Error writing to file: {}", e);
                }

                if opts.loglevel_info() {
                    let now: DateTime<chrono::Local> = chrono::Local::now();
                    print!("[{}] {}: {}", now, path.to_str().unwrap(), hashline);
                }
            }
        }
    }

    if opts.loglevel_info() {
        let now: DateTime<chrono::Local> = chrono::Local::now();
        println!("[{}] Directory {} Updated", now, path.to_str().unwrap());
    }
}

fn calculate_hash(path: String, workdir: &PathBuf, opts: &super::util::Options) -> String {
    let output = Command::new(format!("{}sum", opts.algorithm)).arg(path).current_dir(workdir).output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

struct DirWalker {
    current_files: Vec<PathBuf>,
    current_directories: Vec<PathBuf>,
    unfinished_read: String,
    subdir_mode: bool
}

impl DirWalker {
    pub fn new(start_directory: &PathBuf, subdir_mode: bool) -> DirWalker {
        let dir_entries = fs::read_dir(start_directory).unwrap();
        let mut files: Vec<PathBuf> = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();

        for entry in dir_entries {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();

            if metadata.is_dir() {
                dirs.push(entry.path());
            }
            if metadata.is_file() {
                files.push(entry.path());
            }
        }

        DirWalker{
            current_files: files,
            current_directories: dirs,
            unfinished_read: String::new(),
            subdir_mode
        }
    }
}

impl Iterator for DirWalker {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.current_files.is_empty() {
            let filepath = self.current_files.pop().unwrap();

            if self.subdir_mode {
                let path_string = filepath.to_string_lossy().to_string();
                let path_string = &path_string[2..];
                let position = path_string.find("/").unwrap();
                let path_string = format!(".{}", path_string[position..].to_string());
                let filepath = PathBuf::from(path_string);
                return Some(filepath);
            }

            return Some(filepath);
        }

        if !self.current_directories.is_empty() {
            let dirpath = self.current_directories.pop().unwrap();

            let dir_entries = fs::read_dir(dirpath).unwrap();
            let mut files: Vec<PathBuf> = Vec::new();
            let mut dirs: Vec<PathBuf> = Vec::new();

            for entry in dir_entries {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();

                if metadata.is_dir() {
                    dirs.push(entry.path());
                }
                if metadata.is_file() {
                    files.push(entry.path());
                }
            }

            self.current_files.append(&mut files);
            self.current_directories.append(&mut dirs);

            return self.next();
        }

        return None;
    }
}

impl Read for DirWalker {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut i = 0;

        if self.unfinished_read.len() > 0 {
            loop {
                if i >= buf.len() || i >= self.unfinished_read.len() {
                    break;
                }

                buf[i] = self.unfinished_read.as_bytes()[i];
                i += 1;
            }

            if i < self.unfinished_read.len() {
                self.unfinished_read = self.unfinished_read[0..i].to_string();
            }

            return Ok(i);
        }

        let path: Option<PathBuf> = self.next();

        match path {
            None => Ok(0),
            Some(path) => {
                let path_str = format!("{}\n", path.to_str().unwrap());

                loop {
                    if i >= buf.len() || i >= path_str.len() {
                        break;
                    }


                    buf[i] = path_str.as_bytes()[i];
                    i += 1;
                }

                if i < path_str.len() {
                    self.unfinished_read = path_str[0..i].to_string();
                }

                Ok(i)
            }
        }
    }
}