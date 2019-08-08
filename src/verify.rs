//! This module implements the verify mode

extern crate chrono;
extern crate regex;
extern crate termios;
extern crate threadpool;

use std::borrow::Borrow;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
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

    if !opts.subdir_mode {
        // execute in directory

        if opts.loglevel_progress() {
            let mut termios_noecho = termios::Termios::from_fd(0).unwrap();
            termios_noecho.c_lflag &= !termios::ECHO;
            termios::tcsetattr(0, termios::TCSANOW, &termios_noecho).unwrap();
            println!();
        }

        verify_directory(
            &PathBuf::from(&opts.folder),
            Arc::new(known_good_path),
            Arc::new(to_check_path),
            Arc::new(opts),
            1,
            0,
        );
    } else {
        // iterate over subdirs and spawn verify_directory threads

        let (dirs_to_process, longest_folder) =
            gather_directories_to_process(&opts, &already_checked);

        if opts.loglevel_progress() {
            let mut termios_noecho = termios::Termios::from_fd(0).unwrap();
            termios_noecho.c_lflag &= !termios::ECHO;
            termios::tcsetattr(0, termios::TCSANOW, &termios_noecho).unwrap();
            for _ in 0..dirs_to_process.len() {
                println!();
            }
        }

        match opts.num_threads {
            0 => execute_threads_unlimited(
                opts,
                known_good_path,
                to_check_path,
                dirs_to_process,
                longest_folder,
            ),
            _ => execute_threads_limited(
                opts,
                known_good_path,
                to_check_path,
                dirs_to_process,
                longest_folder,
            ),
        }
    }
}

/// Reads all directories in the working directory and compares them with already checked directories.
/// Ignores directories that don't contain an _algorithm_sum.txt file.
/// Returns unchecked directories and the number of characters in the name of the directory with the longest name.
///
/// # Arguments
/// * `opts` Options object containing the working directory
/// * `already_checked` Vector of already checked directory paths
fn gather_directories_to_process(
    opts: &super::util::Options,
    already_checked: &[PathBuf],
) -> (Vec<PathBuf>, usize) {
    let dir_entries = fs::read_dir(&opts.folder).unwrap();
    let mut dirs_to_process = Vec::new();
    let mut longest_folder = 0;

    for entry in dir_entries {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();

        if metadata.is_dir() && !already_checked.contains(&entry.path()) {
            let sum_txt_path = fs::metadata(format!(
                "{}/{}sum.txt",
                entry.path().to_str().unwrap(),
                &opts.algorithm
            ));
            if let Ok(path) = sum_txt_path {
                if path.is_file() {
                    dirs_to_process.push(entry.path());

                    let len = entry.path().to_str().unwrap().len();
                    if len > longest_folder {
                        longest_folder = len;
                    }
                }
            }
        }
    }

    (dirs_to_process, longest_folder)
}

/// Starts a thread for every directory in dirs_to_process and launches them all at once.
/// Waits for them to finish.
///
/// # Arguments
/// * `opts` Options object
/// * `known_good_path` Path to the text file containing all checked and good directories
/// * `to_check_path` Path to the text file containing all checked and bad directories
/// * `dirs_to_process` Vector of directory paths that have to be checked
/// * `longest_folder` Number of characters in the name of the longest folder
fn execute_threads_unlimited(
    opts: super::util::Options,
    known_good_path: String,
    to_check_path: String,
    dirs_to_process: Vec<PathBuf>,
    longest_folder: usize,
) {
    let mut thread_handles = Vec::new();
    let mut print_line = 1;
    let opts = Arc::new(opts);
    let known_good_path = Arc::new(known_good_path);
    let to_check_path = Arc::new(to_check_path);

    for entry in dirs_to_process {
        let opts = Arc::clone(&opts);
        let known_good_path = Arc::clone(&known_good_path);
        let to_check_path = Arc::clone(&to_check_path);

        let handle = thread::spawn(move || {
            verify_directory(
                &entry,
                known_good_path,
                to_check_path,
                opts,
                print_line,
                longest_folder,
            );
        });
        thread_handles.push(handle);

        print_line += 1;
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
/// * `known_good_path` Path to the text file containing all checked and good directories
/// * `to_check_path` Path to the text file containing all checked and bad directories
/// * `dirs_to_process` Vector of directory paths that have to be checked
/// * `longest_folder` Number of characters in the name of the longest folder
fn execute_threads_limited(
    opts: super::util::Options,
    known_good_path: String,
    to_check_path: String,
    dirs_to_process: Vec<PathBuf>,
    longest_folder: usize,
) {
    let pool = ThreadPool::new(opts.num_threads);
    let mut print_line = 1;
    let opts = Arc::new(opts);
    let known_good_path = Arc::new(known_good_path);
    let to_check_path = Arc::new(to_check_path);

    for entry in dirs_to_process {
        let opts = Arc::clone(&opts);
        let known_good_path = Arc::clone(&known_good_path);
        let to_check_path = Arc::clone(&to_check_path);

        pool.execute(move || {
            verify_directory(
                &entry,
                known_good_path,
                to_check_path,
                opts,
                print_line,
                longest_folder,
            );
        });

        print_line += 1;
    }

    pool.join();
}

/// Verifies the integrity of a directory
///
/// # Arguments
///
/// * `workdir` Path to the directory that should be verified
/// * `known_good_path` The file the workdir path gets appended to if the directory is verified to be good
/// * `to_check_path` The file the workdir path gets appended to if the directory is not verified to be good
/// * `opts` An Options object containing information about the program behavior
/// * `print_line` The line to print progressbar and messages to. Only used in loglevel progress.
/// * `longest_folder` Number of characters in the name of the longest folder, determines how many spaces are padded
fn verify_directory(
    workdir: &PathBuf,
    known_good_path: Arc<String>,
    to_check_path: Arc<String>,
    opts: Arc<super::util::Options>,
    print_line: u32,
    longest_folder: usize,
) {
    if opts.loglevel_info() {
        let now: DateTime<chrono::Local> = chrono::Local::now();
        println!(
            "[{}] Verifying Directory {}",
            now,
            workdir.to_str().unwrap()
        );
    }

    let mut failed_paths = Vec::new();

    let success = if opts.loglevel_progress() {
        verify_directory_with_progressbar(
            &workdir,
            &opts,
            print_line,
            &mut failed_paths,
            longest_folder,
        )
    } else {
        verify_directory_oneshot(&workdir, &opts, &mut failed_paths)
    };

    if success.is_ok() {
        // every file from _algorithm_sum.txt was correct
        inform_directory_good(&workdir, known_good_path, opts);
    } else {
        // some files from _algorithm_sum.txt were INCORRECT
        inform_directory_bad(&workdir, to_check_path, opts, &failed_paths);
    }
}

/// Append workdir to the text file in to_check_path, print FAILED if in loglevel info or above
/// and append all paths to unexpectedly changed files to to_check_workdir.txt
///
/// # Arguments
/// * `workdir` Path to the directory that was just checked
/// * `to_check_path` Path to the text file containing all checked and bad directories
/// * `opts` The Options object determining subdir_mode and loglevel
/// * `failed_paths` Vector of paths to files that have changed
fn inform_directory_bad(
    workdir: &PathBuf,
    to_check_path: Arc<String>,
    opts: Arc<super::util::Options>,
    failed_paths: &[String],
) {
    if opts.subdir_mode {
        let to_check_path: &String = to_check_path.borrow();

        let mut to_check_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(to_check_path)
            .unwrap();
        if let Err(e) = writeln!(to_check_file, "{}", workdir.to_str().unwrap()) {
            eprintln!("Error writing to file: {}", e);
        }
    }
    if opts.loglevel_info() {
        let now = chrono::Local::now();
        println!(
            "[{}] Directory {} checked: FAILED",
            now,
            workdir.to_str().unwrap()
        );
    }
    let mut to_check_dir = workdir.to_str().unwrap();
    if to_check_dir.len() > 2 {
        to_check_dir = &to_check_dir[2..];
    }
    let bad_hashlines_filepath = format!("to_check_{}.txt", to_check_dir);
    if opts.loglevel_debug() {
        println!("Filepath for Bad Files: {:?}", bad_hashlines_filepath);
    }
    let mut bad_hashlines_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(bad_hashlines_filepath)
        .unwrap();
    for line in failed_paths {
        if let Err(e) = writeln!(bad_hashlines_file, "{}", line) {
            eprintln!("Error writing to file: {}", e);
        }
    }
}

/// Append workdir to the text file in known_good_path and print OK if in loglevel info or above.
///
/// # Arguments
/// * `workdir` Path to the directory that was just checked
/// * `known_good_path` Path to the text file containing all checked and good directories
/// * `opts` The Options object determining subdir_mode and loglevel
fn inform_directory_good(
    workdir: &PathBuf,
    known_good_path: Arc<String>,
    opts: Arc<super::util::Options>,
) {
    if opts.subdir_mode {
        let known_good_path: &String = known_good_path.borrow();

        let mut known_good_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(known_good_path)
            .unwrap();
        if let Err(e) = writeln!(known_good_file, "{}", workdir.to_str().unwrap()) {
            eprintln!("Error writing to file: {}", e);
        }
    }

    if opts.loglevel_info() {
        let now = chrono::Local::now();
        println!("[{}] {}: checked: OK", now, workdir.to_str().unwrap());
    }
}

/// Verifies the integrity of a directory
///
/// # Arguments
///
/// * `workdir` Path to the directory that should be verified
/// * `opts` An Options object containing information about the program behavior
/// * `failed_paths` Reference to a Vector of Paths to files that have changed unexpectedly
fn verify_directory_oneshot(
    workdir: &PathBuf,
    opts: &Arc<super::util::Options>,
    failed_paths: &mut Vec<String>,
) -> Result<(), io::Error> {
    let file_path_re = match super::util::regex_from_opts(&opts) {
        Ok(re) => re,
        Err(e) => panic!(e),
    };
    let mut success = true;

    let file = match OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(format!(
            "{}/{}sum.txt",
            workdir.to_str().unwrap(),
            opts.algorithm
        )) {
        Ok(f) => f,
        Err(e) => panic!(e),
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
                        if opts.loglevel_info() {
                            let now: DateTime<chrono::Local> = chrono::Local::now();
                            println!("[{}] {}: {}", now, workdir.to_str().unwrap(), line);
                        }
                        failed_paths.push(String::from(path));
                        success = false;
                    }
                }
            }
        }
    }

    if success {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Some files changed unexpectedly",
        ))
    }
}

/// Verifies the integrity of a directory and printing a progressbar along the way
///
/// # Arguments
///
/// * `workdir` Path to the directory that should be verified
/// * `opts` An Options object containing information about the program behavior
/// * `print_line` Number of lines to scroll up before printing the progressbar
/// * `failed_paths` Reference to a Vector of Paths to files that have changed unexpectedly
/// * `longest_folder` Number of characters in the name of the longest folder
fn verify_directory_with_progressbar(
    workdir: &PathBuf,
    opts: &Arc<super::util::Options>,
    print_line: u32,
    failed_paths: &mut Vec<String>,
    longest_folder: usize,
) -> Result<(), io::Error> {
    let mut processed_bytes: u64 = 0;
    let file_path_re = match super::util::regex_from_opts(&opts) {
        Ok(re) => re,
        Err(e) => panic!(e),
    };

    let all_bytes = count_bytes_from_txt(workdir, opts, &file_path_re);

    print_progress(
        all_bytes,
        processed_bytes,
        print_line,
        &workdir,
        longest_folder,
    )?;

    let file = match OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(format!(
            "{}/{}sum.txt",
            workdir.to_str().unwrap(),
            opts.algorithm
        )) {
        Ok(f) => f,
        Err(e) => panic!(e),
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

                print_progress(
                    all_bytes,
                    processed_bytes,
                    print_line,
                    &workdir,
                    longest_folder,
                )?;
            }
        }
    }

    if failed_paths.is_empty() {
        print_message(print_line, "checked: OK", &workdir)?;
        Ok(())
    } else {
        print_message(print_line, "checked: FAILED", &workdir)?;
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Some files changed unexpectedly",
        ))
    }
}

/// Reads all files from an _algorithm_sum.txt and accumulates all bytes
///
/// # Arguments
/// * `workdir` PathBuf to the current working directory with an _algorithm_sum.txt inside
/// * `opts` The Options object containing the chosen algorithm
/// * `file_path_re` Regex used to extrapolate the filepath from the line containing filepath and hash
fn count_bytes_from_txt(
    workdir: &PathBuf,
    opts: &Arc<super::util::Options>,
    file_path_re: &regex::Regex,
) -> u64 {
    let mut all_bytes = 0;

    let file = match OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(format!(
            "{}/{}sum.txt",
            workdir.to_str().unwrap(),
            opts.algorithm
        )) {
        Ok(f) => f,
        Err(e) => panic!(e),
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

    all_bytes
}

/// Produce a String containing workdir, progress percentage and progress bar, then printing it with print_message
///
/// # Arguments
/// * `all_bytes` Number of bytes in this working directory that are listed in _algorithm_sum.txt
/// * `processed_bytes` Number of already processed bytes
/// * `line` Number of lines to scroll up before printing the message
/// * `workdir` PathBuf to the current working directory, which is printed before the message
/// * `longest_folder` Number of characters in the name of the longest folder, determines how many spaces are padded
fn print_progress(
    all_bytes: u64,
    processed_bytes: u64,
    line: u32,
    workdir: &PathBuf,
    longest_folder: usize,
) -> Result<(), io::Error> {
    let progress = processed_bytes as f64 / all_bytes as f64;
    let mut message = String::new();

    let mut i = workdir.to_str().unwrap().len();
    while i < longest_folder {
        message = format!("{} ", message);
        i += 1;
    }

    message = format!("{} {:03.2}% ", message, progress * 100.0);

    let progress_bar = 60.0 * progress;
    for i in 0..60 {
        if (f64::from(i)) < progress_bar {
            message = format!("{}#", message);
        } else {
            message = format!("{}_", message);
        }
    }

    print_message(line, &message, workdir)
}

/// Print a message N lines above the current cursor.
/// Cursor position is saved and restored after this operation.
/// The line is cleared before printing.
///
/// # Arguments
/// * `line` Number of lines to scroll up before printing the message
/// * `message` String to print
/// * `workdir` PathBuf to the current working directory, which is printed before the message
fn print_message(line: u32, message: &str, workdir: &PathBuf) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(b"\x1b[s")?;
    write!(handle, "\x1b[{}A\x1b[2K", line)?;
    write!(handle, "{}: {}", workdir.to_str().unwrap(), message)?;
    handle.write_all(b"\x1b[u")?;
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

    already_checked.append(&mut super::util::read_paths_from_file(known_good_path));
    already_checked.append(&mut super::util::read_paths_from_file(to_check_path));

    already_checked
}
