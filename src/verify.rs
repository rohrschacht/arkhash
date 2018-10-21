//! This module implements the verify mode

extern crate chrono;
extern crate threadpool;
extern crate termios;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{BufReader, BufRead, Write, self};
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

    let termios_original = termios::Termios::from_fd(0).unwrap();
    let mut termios_noecho = termios_original.clone();
    termios_noecho.c_lflag &= !termios::ECHO;

    // no-subdir: execute in directory
    // subdir: iterate over subdirs and spawn verify_directory threads, if path not in vec
    match opts.subdir_mode {
        false => {
            if opts.loglevel_progress() {
                let _unused = termios::tcsetattr(0, termios::TCSANOW, &termios_noecho).unwrap();
                println!();
            }
            verify_directory(PathBuf::from(&opts.folder), known_good_path, to_check_path, opts, 1, 0);
        },
        true => {
            let dir_entries = fs::read_dir(&opts.folder).unwrap();
            let mut dirs_to_process = Vec::new();
            let mut longest_folder = 0;

            for entry in dir_entries {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();

                if metadata.is_dir() {
                    dirs_to_process.push(entry.path());

                    let len = entry.path().to_str().unwrap().len();
                    if len > longest_folder {
                        longest_folder = len;
                    }
                }
            }

            let dirs_to_process: Vec<PathBuf> = dirs_to_process.into_iter().filter(|x| !already_checked.contains(x)).collect();

            if opts.loglevel_progress() {
                let _unused = termios::tcsetattr(0, termios::TCSANOW, &termios_noecho).unwrap();
                for _ in 0..dirs_to_process.len() {
                    println!();
                }
            }

            let mut print_line = 1;

            match opts.num_threads {
                0 => {
                    let mut thread_handles = Vec::new();

                    for entry in dirs_to_process {
                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        let thread_known_good_path = known_good_path.clone();
                        let thread_to_check_path = to_check_path.clone();
                        let thread_print_line = print_line.clone();
                        let handle = thread::spawn(move || {
                            verify_directory(thread_path, thread_known_good_path, thread_to_check_path, thread_opts, thread_print_line, longest_folder);
                        });
                        thread_handles.push(handle);

                        print_line += 1;
                    }

                    for handle in thread_handles {
                        handle.join().unwrap();
                    }
                },
                _ => {
                    let pool = ThreadPool::new(opts.num_threads);

                    for entry in dirs_to_process {
                        let thread_path = entry.clone();
                        let thread_opts = opts.clone();
                        let thread_known_good_path = known_good_path.clone();
                        let thread_to_check_path = to_check_path.clone();
                        let thread_print_line = print_line.clone();
                        pool.execute(move || {
                            verify_directory(thread_path, thread_known_good_path, thread_to_check_path, thread_opts, thread_print_line, longest_folder);
                        });

                        print_line += 1;
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
fn verify_directory(workdir: PathBuf, known_good_path: String, to_check_path: String, opts: super::util::Options, print_line: u32, longest_folder: usize) {
    if opts.loglevel_info() {
        let now: DateTime<chrono::Local> = chrono::Local::now();
        println!("[{}] Verifying Directory {}", now, workdir.to_str().unwrap());
    }

    let mut failed_paths = Vec::new();

    let success = match opts.loglevel_progress() {
        true => verify_directory_with_progressbar(&workdir, &opts, &print_line, &mut failed_paths, longest_folder),
        false => verify_directory_oneshot(&workdir, &opts, &mut failed_paths)
    };

    if success.is_ok() {
        // every file from _algorithm_sum.txt was correct

        if opts.subdir_mode {
            let mut known_good_file = OpenOptions::new().create(true).append(true).open(known_good_path).unwrap();
            if let Err(e) = writeln!(known_good_file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }
        }

        if opts.loglevel_info() {
            let now = chrono::Local::now();
            println!("[{}] {}: checked: OK", now, workdir.to_str().unwrap());
        }
    } else {
        // some files from _algorithm_sum.txt were INCORRECT

        if opts.subdir_mode {
            let mut to_check_file = OpenOptions::new().create(true).append(true).open(to_check_path).unwrap();
            if let Err(e) = writeln!(to_check_file, "{}", workdir.to_str().unwrap()) {
                eprintln!("Error writing to file: {}", e);
            }
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

        for line in failed_paths {
            if let Err(e) = writeln!(bad_hashlines_file, "{}", line) {
                eprintln!("Error writing to file: {}", e);
            }
        }
    }
}

fn verify_directory_oneshot(workdir: &PathBuf, opts: &super::util::Options, failed_paths: &mut Vec<String>) -> Result<(), io::Error> {
    let child = Command::new(format!("{}sum", opts.algorithm)).arg("-c").arg("--quiet").arg(format!("{}sum.txt", opts.algorithm))
        .current_dir(&workdir).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();

    if let Ok(mut child) = child {
        // The _algorithm_sum command can be successfully executed in workdir

        let reader = BufReader::new(child.stdout.take().unwrap());

        for line in reader.lines() {
            match line {
                Err(_) => continue,
                Ok(line) => {
                    if opts.loglevel_info() {
                        let now: DateTime<chrono::Local> = chrono::Local::now();
                        println!("[{}] {}: {}", now, workdir.to_str().unwrap(), line);
                    }

                    failed_paths.push(line);
                }
            }
        }

        let exit_status = child.wait().unwrap();

        if exit_status.success() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "Some files changed unexpectedly"))
        }

    } else {
        // The _algorithm_sum command can NOT be successfully executed in workdir
        if opts.loglevel_info() {
            let now = chrono::Local::now();
            println!("[{}] Directory {}: Permission Denied", now, workdir.to_str().unwrap());
        }

        Err(io::Error::new(io::ErrorKind::InvalidData, "Some files changed unexpectedly"))
    }
}

fn verify_directory_with_progressbar(workdir: &PathBuf, opts: &super::util::Options, print_line: &u32, failed_paths: &mut Vec<String>, longest_folder: usize) -> Result<(), io::Error> {
    let mut all_bytes: u64 = 5;
    let mut processed_bytes: u64 = 0;
    let file_path_re = match super::util::regex_from_opts(&opts) {
        Ok(re) => re,
        Err(e) => panic!(e)
    };

    let file = match OpenOptions::new().read(true).append(true).create(true).open(format!("{}/{}sum.txt", workdir.to_str().unwrap(), opts.algorithm)) {
        Ok(f) => f,
        Err(e) => panic!(e)
    };

    for line in BufReader::new(file).lines() {
        if let Ok(line) = line {
            if let Some(captures) = file_path_re.captures(&line) {
                let path = &captures[2];
                let metadata = fs::metadata(format!("{}/{}", workdir.to_str().unwrap(), path));
                if let Ok(metadata) = metadata {
                    all_bytes += metadata.len();
                }
            }
        }
    }

    print_progress(&all_bytes, &processed_bytes, &print_line, &workdir, longest_folder)?;

    let file = match OpenOptions::new().read(true).append(true).create(true).open(format!("{}/{}sum.txt", workdir.to_str().unwrap(), opts.algorithm)) {
        Ok(f) => f,
        Err(e) => panic!(e)
    };

    for line in BufReader::new(file).lines() {
        if let Ok(line) = line {
            if let Some(captures) = file_path_re.captures(&line) {
                let hash = &captures[1];
                let path = &captures[2];

                let mut new_hash = super::util::calculate_hash(String::from(path), &workdir, &opts);
                new_hash.pop();
                if let Some(new_captures) = file_path_re.captures(&new_hash) {
                    let new_hash = &new_captures[1];
                    if new_hash != hash {
                        failed_paths.push(String::from(path));
                    }
                }

                let metadata = fs::metadata(format!("{}/{}", workdir.to_str().unwrap(), path));
                if let Ok(metadata) = metadata {
                    processed_bytes += metadata.len();
                }

                print_progress(&all_bytes, &processed_bytes, &print_line, &workdir, longest_folder)?;
            }
        }
    }

    if failed_paths.is_empty() {
        print_message(&print_line, "checked: OK", &workdir)?;
        Ok(())
    } else {
        print_message(&print_line, "checked: FAILED", &workdir)?;
        Err(io::Error::new(io::ErrorKind::InvalidData, "Some files changed unexpectedly"))
    }
}

fn print_progress(all_bytes: &u64, processed_bytes: &u64, line: &u32, workdir: &PathBuf, longest_folder: usize) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let progress = *processed_bytes as f64 / *all_bytes as f64;
    handle.write(b"\x1b[s")?;
    write!(handle, "\x1b[{}A\x1b[2K", line)?;
    write!(handle, "{}", workdir.to_str().unwrap())?;
    let mut i = workdir.to_str().unwrap().len();
    while i < longest_folder {
        handle.write(b" ")?;
        i += 1;
    }
    write!(handle, ": {:03.2}% ", progress * 100.0)?;
    let progress_bar = 60.0 * progress;
    for i in 0..60 {
        if (i as f64) < progress_bar {
            handle.write(b"#")?;
        } else {
            handle.write(b"_")?;
        }
    }
    handle.write(b"\x1b[u")?;
    io::stdout().flush()
}

fn print_message(line: &u32, message: &str, workdir: &PathBuf) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle.write(b"\x1b[s")?;
    write!(handle, "\x1b[{}A\x1b[2K", line)?;
    write!(handle, "{}: {}", workdir.to_str().unwrap(), message)?;
    handle.write(b"\x1b[u")?;
    io::stdout().flush()
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