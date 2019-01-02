//! This module implements the update functionality

extern crate chrono;
extern crate threadpool;

use std::fs::{self, OpenOptions};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::thread;

use self::chrono::DateTime;

use self::threadpool::ThreadPool;

/// Updates the _algorithm_sum.txt files of some directories
///
/// # Arguments
///
/// * `opts` An Options object containing information about the program behavior
pub fn update_directories(opts: &super::util::Options) {
    if !opts.subdir_mode {
        update_hashsums(&PathBuf::from(&opts.folder), &opts)
    } else {
        let dirs_to_process = gather_directories_to_process(&opts);

        match opts.num_threads {
            0 => execute_threads_unlimited(&opts, dirs_to_process),
            _ => execute_threads_limited(&opts, dirs_to_process),
        }
    }
}

/// Reads all directories in the working directory.
/// Ignores all directories listed in .hfignore
///
/// # Arguments
/// * `opts` Options object containing the working directory
fn gather_directories_to_process(opts: &super::util::Options) -> Vec<PathBuf> {
    let dir_entries = fs::read_dir(&opts.folder).unwrap();
    let to_ignore = read_to_ignore(&opts);

    if opts.loglevel_debug() {
        println!("Dirs to ignore: {:?}", to_ignore);
    }

    let mut dirs_to_process = Vec::new();
    for entry in dir_entries {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();

        if metadata.is_dir() && !to_ignore.contains(&entry.path()) {
            dirs_to_process.push(entry.path());
        }
    }

    dirs_to_process
}

/// Reads the .hfignore file and returns a Vector of directories that should be ignored when updating hashes.
///
/// # Arguments
/// * `opts` Options object containing the working directory
fn read_to_ignore(opts: &super::util::Options) -> Vec<PathBuf> {
    let to_ignore = super::util::read_paths_from_file(&format!("{}{}", &opts.folder, "/.hfignore"));
    let mut to_ignore_prepended = Vec::new();

    for path in to_ignore {
        if !path.to_str().unwrap().starts_with("./") {
            let new_path = PathBuf::from(format!("./{}", path.to_str().unwrap()));
            to_ignore_prepended.push(new_path);
        } else {
            to_ignore_prepended.push(path);
        }
    }

    to_ignore_prepended
}

/// Starts a thread for every directory in dirs_to_process and launches them all at once.
/// Waits for them to finish.
///
/// # Arguments
/// * `opts` Options object
/// * `dirs_to_process` Vector of directory paths that have to be updated
fn execute_threads_unlimited(opts: &super::util::Options, dirs_to_process: Vec<PathBuf>) {
    let mut thread_handles = Vec::new();
    for entry in dirs_to_process {
        if opts.loglevel_info() {
            let now: DateTime<chrono::Local> = chrono::Local::now();
            println!("[{}] Updating Directory {}", now, entry.to_str().unwrap());
        }

        let thread_path = entry.clone();
        let thread_opts = opts.clone();
        let handle = thread::spawn(move || {
            update_hashsums(&thread_path, &thread_opts);
        });
        thread_handles.push(handle);
    }
    for handle in thread_handles {
        handle.join().unwrap();
    }
}

/// Starts a thread for every directory in dirs_to_process and launches opts.num_threads of them in parallel.
/// When a thread finished its work, the next one will be launched.
/// Waits for them to finish.
///
/// # Arguments
/// * `opts` Options object
/// * `dirs_to_process` Vector of directory paths that have to be updated
fn execute_threads_limited(opts: &super::util::Options, dirs_to_process: Vec<PathBuf>) {
    let pool = ThreadPool::new(opts.num_threads);

    for entry in dirs_to_process {
        if opts.loglevel_info() {
            let now: DateTime<chrono::Local> = chrono::Local::now();
            println!("[{}] Updating Directory {}", now, entry.to_str().unwrap());
        }

        let thread_path = entry.clone();
        let thread_opts = opts.clone();
        pool.execute(move || {
            update_hashsums(&thread_path, &thread_opts);
        });
    }

    pool.join();
}

/// Updates the _algorithm_sum.txt in a directory
///
/// # Arguments
///
/// * `path` The path to the directory that is going to be updated
/// * `opts` An Options object containing information about the program behavior
fn update_hashsums(path: &PathBuf, opts: &super::util::Options) {
    let dirwalker = super::util::DirWalker::new(&path, opts.subdir_mode);
    let reader = BufReader::new(dirwalker);

    let filter = super::filter::Filter::new(reader, path.to_str().unwrap(), &opts);

    if let Ok(filter) = filter {
        let mut filepath = path.clone();
        filepath.push(format!("{}sum.txt", opts.algorithm));
        let mut file = OpenOptions::new().create(true).append(true).open(filepath);

        if let Ok(mut file) = file {
            for line in filter {
                let hashline = super::util::calculate_hash(line, &path, &opts);

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
