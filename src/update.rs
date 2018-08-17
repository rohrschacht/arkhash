//! This module implements the update functionality

extern crate chrono;
extern crate threadpool;

use std::fs::{self, OpenOptions};
use std::path::{PathBuf};
use std::io::{Read, Error, BufReader, Write};
use std::process::Command;
use std::thread;

use self::chrono::DateTime;

use self::threadpool::ThreadPool;


/// Updates the _algorithm_sum.txt files of some directories
///
/// # Arguments
///
/// * `opts` An Options object containing information about the program behavior
pub fn update_directories(opts: super::util::Options) {
    match opts.subdir_mode {
        false => update_hashsums(PathBuf::from(&opts.folder), opts),
        true => {
            let dir_entries = fs::read_dir(&opts.folder).unwrap();
            let mut dirs_to_process = Vec::new();

            for entry in dir_entries {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();

                if metadata.is_dir() {
                    dirs_to_process.push(entry.path());
                }
            }

            match opts.num_threads {
                0 => {
                    let mut thread_handles = Vec::new();

                    for entry in dirs_to_process {
                        if opts.loglevel_info() {
                            let now: DateTime<chrono::Local> = chrono::Local::now();
                            println!("[{}] Updating Directory {}", now, entry.to_str().unwrap());
                        }

                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        let handle = thread::spawn(|| {
                            update_hashsums(thread_path, thread_opts);
                        });
                        thread_handles.push(handle);
                    }

                    for handle in thread_handles {
                        handle.join().unwrap();
                    }
                },
                _ => {
                    let pool = ThreadPool::new(opts.num_threads);

                    for entry in dirs_to_process {
                        if opts.loglevel_info() {
                            let now: DateTime<chrono::Local> = chrono::Local::now();
                            println!("[{}] Updating Directory {}", now, entry.to_str().unwrap());
                        }

                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        pool.execute(|| {
                            update_hashsums(thread_path, thread_opts);
                        });
                    }

                    pool.join();
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

/// Call _algorithm_sum with the path of a file to get the hashsum.
///
/// # Arguments
///
/// * `path` Path to the file to be hashed, relative to the workdir
/// * `workdir` Path to the wanted working directory
/// * `opts` A reference to an Options object containing information about the program behavior
///
/// # Returns
///
/// A String containing the output of the _algorithm_sum command.
fn calculate_hash(path: String, workdir: &PathBuf, opts: &super::util::Options) -> String {
    let output = Command::new(format!("{}sum", opts.algorithm)).arg(path).current_dir(workdir).output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// An Object that returns Paths to all the files in all folders recursively (like find)
///
/// DirWalker implements Iterator and Read for this behavior
struct DirWalker {
    /// A Buffer for the currently known files
    current_files: Vec<PathBuf>,
    /// A Buffer for the directories that have to be scanned recursively
    current_directories: Vec<PathBuf>,
    /// A Buffer for the filepath that was only partially read
    unfinished_read: String,
    /// Whether or not the first directory should be stripped from the filepath
    subdir_mode: bool
}

impl DirWalker {
    /// Create a new DirWalker object
    ///
    /// # Arguments
    ///
    /// * `start_directory` Path to the directory that should be scanned
    /// * `subdir_mode` Whether or not the first directory should be stripped from the filepath
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