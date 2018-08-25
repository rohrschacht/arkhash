//! This module implements the verify mode

extern crate chrono;
extern crate threadpool;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Write};
use std::fs::{self, OpenOptions};
use std::thread;

use self::chrono::{DateTime, Datelike};

use self::threadpool::ThreadPool;


/// Verifies the integrity of some directories
///
/// # Arguments
///
/// * `opts` An Options object containing information about the program behavior
pub fn verify_directories(opts: super::util::Options) {
    let now = chrono::Local::now();
    let known_good_path = format!("known_good_{}_{}.txt", now.month(), now.year());
    let to_check_path = format!("to_check_{}_{}.txt", now.month(), now.year());

    // read every line from known_good_path and to_check_path to vec
    let already_checked = read_already_checked(&known_good_path, &to_check_path);
    if opts.loglevel_debug() {
        println!("Already checked subdirs: {:?}", already_checked);
    }

    // no-subdir: execute in directory
    // subdir: iterate over subdirs and spawn verify_directory threads, if path not in vec
    match opts.subdir_mode {
        false => verify_directory(PathBuf::from(&opts.folder), known_good_path, to_check_path, opts),
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

            let dirs_to_process: Vec<PathBuf> = dirs_to_process.into_iter().filter(|x| !already_checked.contains(x)).collect();

            match opts.num_threads {
                0 => {
                    let mut thread_handles = Vec::new();

                    for entry in dirs_to_process {
                        if opts.loglevel_info() {
                            let now: DateTime<chrono::Local> = chrono::Local::now();
                            println!("[{}] Verifying Directory {}", now, entry.to_str().unwrap());
                        }

                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        let thread_known_good_path = known_good_path.clone();
                        let thread_to_check_path = to_check_path.clone();
                        let handle = thread::spawn(|| {
                            verify_directory(thread_path, thread_known_good_path, thread_to_check_path, thread_opts);
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
                            println!("[{}] Verifying Directory {}", now, entry.to_str().unwrap());
                        }

                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        let thread_known_good_path = known_good_path.clone();
                        let thread_to_check_path = to_check_path.clone();
                        pool.execute(|| {
                            verify_directory(thread_path, thread_known_good_path, thread_to_check_path, thread_opts);
                        });
                    }

                    pool.join();
                }
            }
        }
    }
}

/// Verifies the integrity of a directory
///
/// # Arguments
///
/// * `workdir` Path to the directory that should be verified
/// * `known_good_path` The file the workdir path gets appended to if the directory is verified to be good
/// * `to_check_path` The file the workdir path gets appended to if the directory is not verified to be good
/// * `opts` An Options object containing information about the program behavior
fn verify_directory(workdir: PathBuf, known_good_path: String, to_check_path: String, opts: super::util::Options) {
    let child = Command::new(format!("{}sum", opts.algorithm)).arg("-c").arg("--quiet").arg(format!("{}sum.txt", opts.algorithm))
        .current_dir(&workdir).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();

    if let Ok(mut child) = child {
        // The _algorithm_sum command can be successfully executed in workdir

        let mut output = Vec::new();
        let reader = BufReader::new(child.stdout.take().unwrap());

        for line in reader.lines() {
            match line {
                Err(_) => continue,
                Ok(line) => {
                    if opts.loglevel_info() {
                        let now: DateTime<chrono::Local> = chrono::Local::now();
                        println!("[{}] {}: {}", now, workdir.to_str().unwrap(), line);
                    }

                    output.push(line);
                }
            }
        }

        let exit_status = child.wait().unwrap();

        if exit_status.success() {
            // every file from _algorithm_sum.txt was correct

            let mut known_good_file = OpenOptions::new().create(true).append(true).open(known_good_path).unwrap();
            if let Err(e) = writeln!(known_good_file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }

            if opts.loglevel_info() {
                let now = chrono::Local::now();
                println!("[{}] {}: checked: OK", now, workdir.to_str().unwrap());
            }
        } else {
            // some files from _algorithm_sum.txt were INCORRECT

            let mut to_check_file = OpenOptions::new().create(true).append(true).open(to_check_path).unwrap();
            if let Err(e) = writeln!(to_check_file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }

            if opts.loglevel_info() {
                let now = chrono::Local::now();
                println!("[{}] Directory {} checked: FAILED", now, workdir.to_str().unwrap());
            }

            let mut to_check_dir = workdir.to_str().unwrap();
            if to_check_dir.len() > 2 {
                to_check_dir = &to_check_dir[2..];
            }

            let bad_hashlines_filepath = format!("to_check_{}.txt", to_check_dir);
            if opts.loglevel_debug() {
                println!("Filepath for Bad Files: {:?}", bad_hashlines_filepath);
            }
            
            let mut bad_hashlines_file = OpenOptions::new().create(true).append(true).open(bad_hashlines_filepath).unwrap();

            for line in output {
                if let Err(e) = writeln!(bad_hashlines_file, "{}", line) {
                    eprintln!("Error writing to file: {}", e);
                }
            }
        }
    } else {
        // The _algorithm_sum command can NOT be successfully executed in workdir
        if opts.loglevel_info() {
            let now = chrono::Local::now();
            println!("[{}] Directory {}: Permission Denied", now, workdir.to_str().unwrap());
        }
    }
}

/// Build up a vec containing the paths to directories that were already checked
///
/// # Arguments
///
/// * `known_good_path` Path to the file containing directories that are known to be good
/// * `to_check_path` Path to the file containing directories that are known to be bad
fn read_already_checked(known_good_path: &str, to_check_path: &str) -> Vec<PathBuf> {
    let mut already_checked = Vec::new();

    already_checked.append(&mut read_paths_from_file(known_good_path));
    already_checked.append(&mut read_paths_from_file(to_check_path));

    already_checked
}

/// Read paths line by line from a file and return them in a vec
///
/// # Arguments
///
/// * `filepath` Path to the file to be read
fn read_paths_from_file(filepath: &str) -> Vec<PathBuf> {
    let mut vec = Vec::new();

    let file = OpenOptions::new().read(true).open(filepath);
    if let Ok(file) = file {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                vec.push(PathBuf::from(line));
            }
        }
    }

    vec
}