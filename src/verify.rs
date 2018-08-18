extern crate chrono;
extern crate threadpool;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Write};
use std::fs::{self, OpenOptions};
use std::thread;

use self::chrono::{DateTime, Datelike};

use self::threadpool::ThreadPool;


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

fn verify_directory(workdir: PathBuf, known_good_path: String, to_check_path: String, opts: super::util::Options) {
    let child = Command::new(format!("{}sum", opts.algorithm)).arg("-c").arg("--quiet").arg(format!("{}sum.txt", opts.algorithm)).current_dir(&workdir).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();

    if let Ok(mut child) = child {
        let mut output = Vec::new();

        if opts.loglevel_info() {
            let reader = BufReader::new(child.stdout.take().unwrap());

            for line in reader.lines() {
                match line {
                    Err(_) => continue,
                    Ok(line) => {
                        let now: DateTime<chrono::Local> = chrono::Local::now();
                        println!("[{}] {}: {}", now, workdir.to_str().unwrap(), line);

                        output.push(line);
                    }
                }
            }
        }

        let exit_status = child.wait().unwrap();

        if exit_status.success() {
            let mut file = OpenOptions::new().create(true).append(true).open(known_good_path).unwrap();
            if let Err(e) = writeln!(file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }

            if opts.loglevel_info() {
                let now = chrono::Local::now();
                println!("[{}] {}: checked: OK", now, workdir.to_str().unwrap());
            }
        } else {
            let mut file = OpenOptions::new().create(true).append(true).open(to_check_path).unwrap();
            if let Err(e) = writeln!(file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }

            if opts.loglevel_info() {
                let now = chrono::Local::now();
                println!("[{}] Directory {} checked: FAILED", now, workdir.to_str().unwrap());
            }

            let bad_hashlines_filepath = format!("to_check_{}.txt", &workdir.to_str().unwrap()[2..]);
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
        if opts.loglevel_info() {
            let now = chrono::Local::now();
            println!("[{}] Directory {}: Permission Denied", now, workdir.to_str().unwrap());
        }
    }
}

fn read_already_checked(known_good_path: &str, to_check_path: &str) -> Vec<PathBuf> {
    let mut already_checked = Vec::new();

    let known_good_file = OpenOptions::new().read(true).open(known_good_path);
    match known_good_file {
        Err(_) => {},
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                match line {
                    Err(_) => {},
                    Ok(line) => already_checked.push(PathBuf::from(line))
                }
            }
        }
    }

    let to_check_file = OpenOptions::new().read(true).open(to_check_path);
    match to_check_file {
        Err(_) => {},
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                match line {
                    Err(_) => {},
                    Ok(line) => already_checked.push(PathBuf::from(line))
                }
            }
        }
    }

    already_checked
}