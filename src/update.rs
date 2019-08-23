//! This module implements the update functionality

extern crate chrono;
extern crate crossbeam_deque;
extern crate num_cpus;

use std::fs::{self, OpenOptions};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use self::chrono::DateTime;

use self::crossbeam_deque::Injector;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;

/// Updates the _algorithm_sum.txt files of some directories
///
/// # Arguments
///
/// * `opts` An Options object containing information about the program behavior
pub fn update_directories(opts: super::util::Options) {
    if !opts.subdir_mode {
        let mut worker_handles = Vec::new();
        let q = Arc::new(Injector::new());
        let producer_finished = Arc::new(AtomicBool::new(false));
        let num_threads = match opts.num_threads {
            0 => num_cpus::get(),
            _ => opts.num_threads,
        };

        let opts = Arc::new(opts);
        let workdir = PathBuf::from(&opts.folder);
        let myq = Arc::clone(&q);

        let handle = thread::spawn(move || {
            update_hashsums(&workdir, opts, myq);
        });

        super::util::execute_workers(
            num_threads,
            Arc::clone(&q),
            Arc::clone(&producer_finished),
            &mut worker_handles,
        );

        handle.join().unwrap();

        producer_finished.store(true, Ordering::Relaxed);

        for handle in worker_handles {
            handle.join().unwrap();
        }
    } else {
        let dirs_to_process = gather_directories_to_process(&opts);

        execute_threads_subdir(opts, dirs_to_process)
    }
}

/// Reads all directories in the working directory.
/// Ignores all directories listed in .arkignore
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

/// Reads the .arkignore file and returns a Vector of directories that should be ignored when updating hashes.
///
/// # Arguments
/// * `opts` Options object containing the working directory
fn read_to_ignore(opts: &super::util::Options) -> Vec<PathBuf> {
    let to_ignore =
        super::util::read_paths_from_file(&format!("{}{}", &opts.folder, "/.arkignore"));
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

/// Starts a thread for every directory in dirs_to_process as a HashTask producer.
/// Launches as many worker threads as opts.num_threads or number of logical cpus.
///
/// # Arguments
/// * `opts` Options object
/// * `dirs_to_process` Vector of directory paths that have to be updated
fn execute_threads_subdir(opts: super::util::Options, dirs_to_process: Vec<PathBuf>) {
    let mut producer_handles = Vec::new();
    let mut worker_handles = Vec::new();
    let opts = Arc::new(opts);
    let q = Arc::new(Injector::new());
    let producer_finished = Arc::new(AtomicBool::new(false));
    let num_threads = match opts.num_threads {
        0 => num_cpus::get(),
        _ => opts.num_threads,
    };

    for entry in dirs_to_process {
        if opts.loglevel_info() {
            let now: DateTime<chrono::Local> = chrono::Local::now();
            println!("[{}] Updating Directory {}", now, entry.to_str().unwrap());
        }

        let opts = Arc::clone(&opts);
        let myq = Arc::clone(&q);

        let handle = thread::spawn(move || {
            update_hashsums(&entry, opts, myq);
        });

        producer_handles.push(handle);
    }

    super::util::execute_workers(
        num_threads,
        Arc::clone(&q),
        Arc::clone(&producer_finished),
        &mut worker_handles,
    );

    for handle in producer_handles {
        handle.join().unwrap();
    }

    producer_finished.store(true, Ordering::Relaxed);

    for handle in worker_handles {
        handle.join().unwrap();
    }
}

/// Updates the _algorithm_sum.txt in a directory
///
/// # Arguments
///
/// * `path` The path to the directory that is going to be updated
/// * `opts` An Options object containing information about the program behavior
/// * `myq` An Injector queue that is used to push the generated hashtasks to the workers and receive the results
fn update_hashsums(
    path: &PathBuf,
    opts: Arc<super::util::Options>,
    myq: Arc<Injector<super::util::HashTask>>,
) {
    if dir_is_empty(path) {
        return;
    }

    let dirwalker = super::util::DirWalker::new(&path, opts.subdir_mode);
    let reader = BufReader::new(dirwalker);

    let filter = super::filter::Filter::new(reader, path.to_str().unwrap(), &opts);

    let (sender, receiver) = channel();

    if let Ok(filter) = filter {
        let mut filepath = path.clone();
        filepath.push(format!("{}sum.txt", opts.algorithm));
        let file = OpenOptions::new().create(true).append(true).open(filepath);

        if let Ok(mut file) = file {
            for line in filter {
                let task = super::util::HashTask {
                    path: line,
                    workdir: PathBuf::from(path),
                    opts: Arc::clone(&opts),
                    cmp: String::new(),
                    result_chan: sender.clone(),
                };

                myq.push(task);
            }

            drop(sender);

            for (hashline, _) in receiver {
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

fn dir_is_empty(path: &PathBuf) -> bool {
    let mut dirwalker = super::util::DirWalker::new(&path, false);
    match dirwalker.next() {
        Some(_) => false,
        None => true,
    }
}
