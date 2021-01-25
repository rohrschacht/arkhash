//! This module implements the verify mode

extern crate chrono;
extern crate crossbeam_deque;
extern crate num_cpus;
extern crate regex;

use std::borrow::Borrow;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use self::chrono::{DateTime, Datelike};

use self::crossbeam_deque::Injector;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};

use super::util::HashError;

/// Verifies the integrity of some directories
///
/// # Arguments
///
/// * `opts` An Options object containing information about the program behavior
///
/// # Returns
/// The exit code the program should return.
pub fn verify_directories(opts: super::util::Options) -> i32 {
    let now = chrono::Local::now();
    let known_good_path = format!("known_good_{}_{}.txt", now.month(), now.year());
    let to_check_path = format!("to_check_{}_{}.txt", now.month(), now.year());

    if !opts.subdir_mode {
        // execute in directory

        if opts.loglevel_progress() {
            super::util::terminal_noecho();
            println!();
        }
        let mut worker_handles = Vec::new();
        let q = Arc::new(Injector::new());
        let producer_finished = Arc::new(AtomicBool::new(false));
        let num_threads = match opts.num_threads {
            0 => num_cpus::get(),
            _ => opts.num_threads,
        };

        let opts = Arc::new(opts);
        let cloned_opts = Arc::clone(&opts);
        let workdir = PathBuf::from(&opts.folder);
        let myq = Arc::clone(&q);
        let (tx, rx) = channel();

        let handle = thread::spawn(move || {
            verify_directory(
                &workdir,
                Arc::new(known_good_path),
                Arc::new(to_check_path),
                cloned_opts,
                1,
                0,
                myq,
                tx,
            );
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

        let mut exit_code = 0;

        for code in rx {
            if code != 0 {
                exit_code = code;
            }
        }

        exit_code
    } else {
        // iterate over subdirs and spawn verify_directory threads
        execute_threads_subdir(
            opts,
            known_good_path,
            to_check_path,
        )
    }
}

/// Reads all directories in the working directory and compares them with already checked directories.
/// Ignores directories that don't contain an _algorithm_sum.txt file.
/// Logs information about known good and known bad directories in info and progress levels.
/// Returns unchecked directories and the number of characters in the name of the directory with the longest name.
/// Also returns a flag indicating if there exist known bad directories.
///
/// # Arguments
/// * `opts` Options object containing the working directory
/// * `known_good_path` Path to the text file containing all checked and good directories
/// * `to_check_path` Path to the text file containing all checked and bad directories
fn gather_directories_to_process(
    opts: &super::util::Options,
    known_good_path: &String,
    to_check_path: &String,
) -> (Vec<PathBuf>, usize, bool) {
    // read every line from known_good_path and to_check_path to vec
    let already_checked_good = super::util::read_paths_from_file(&known_good_path);
    let already_checked_bad = super::util::read_paths_from_file(&to_check_path);
    if opts.loglevel_debug() {
        println!("Already checked subdirs: known good: {:?}, known bad: {:?}", already_checked_good, already_checked_bad);
    }

    if opts.loglevel_info() {
        let now: DateTime<chrono::Local> = chrono::Local::now();
        for dir in already_checked_good.iter().as_ref() {
            println!(
                "[{}] Directory {} already marked known good",
                now,
                dir.to_str().unwrap()
            );
        }
        for dir in already_checked_bad.iter().as_ref() {
            println!(
                "[{}] Directory {} already marked known bad",
                now,
                dir.to_str().unwrap()
            );
        }
    }

    let dir_entries = fs::read_dir(&opts.folder).unwrap();
    let mut dirs_to_process = Vec::new();
    let mut longest_folder = 0;

    for entry in dir_entries {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();

        if metadata.is_dir() {
            if !(already_checked_good.contains(&entry.path()) || already_checked_bad.contains(&entry.path())) {
                let sum_txt_path = fs::metadata(format!(
                    "{}/{}sum.txt",
                    entry.path().to_str().unwrap(),
                    &opts.algorithm
                ));
                if let Ok(path) = sum_txt_path {
                    if path.is_file() {
                        dirs_to_process.push(entry.path());
                    }
                }
            }

            let len = entry.path().to_str().unwrap().len();
            if len > longest_folder {
                longest_folder = len;
            }
        }
    }
    
    if opts.loglevel_progress() {
        for dir in already_checked_good {
            println!();
            print_message_aligned(1, "already known good", dir.to_str().unwrap(), longest_folder).unwrap();
        }
        for dir in already_checked_bad.iter().by_ref() {
            println!();
            print_message_aligned(1, "already known BAD", dir.to_str().unwrap(), longest_folder).unwrap();
        }
    }

    (dirs_to_process, longest_folder, already_checked_bad.is_empty())
}

/// Starts a thread for every directory in dirs_to_process and launches them all at once.
/// Waits for them to finish.
///
/// # Arguments
/// * `opts` Options object
/// * `known_good_path` Path to the text file containing all checked and good directories
/// * `to_check_path` Path to the text file containing all checked and bad directories
///
/// # Returns
/// The exit code the program should return.
fn execute_threads_subdir(
    opts: super::util::Options,
    known_good_path: String,
    to_check_path: String,
) -> i32 {
    let (dirs_to_process, longest_folder, known_bad_empty) =
        gather_directories_to_process(&opts, &known_good_path, &to_check_path);

    if opts.loglevel_progress() {
        super::util::terminal_noecho();
        for _ in 0..dirs_to_process.len() {
            println!();
        }
    }

    let mut producer_handles = Vec::new();
    let mut worker_handles = Vec::new();
    let mut print_line = 1;
    let opts = Arc::new(opts);
    let known_good_path = Arc::new(known_good_path);
    let to_check_path = Arc::new(to_check_path);
    let q = Arc::new(Injector::new());
    let producer_finished = Arc::new(AtomicBool::new(false));
    let num_threads = match opts.num_threads {
        0 => num_cpus::get(),
        _ => opts.num_threads,
    };
    let (tx, rx) = channel();
    let mut exit_code = if known_bad_empty { 0 } else { 2 };

    for entry in dirs_to_process {
        let opts = Arc::clone(&opts);
        let myq = Arc::clone(&q);
        let known_good_path = Arc::clone(&known_good_path);
        let to_check_path = Arc::clone(&to_check_path);
        let tx = tx.clone();

        let handle = thread::spawn(move || {
            verify_directory(
                &entry,
                known_good_path,
                to_check_path,
                opts,
                print_line,
                longest_folder,
                myq,
                tx,
            );
        });

        producer_handles.push(handle);

        print_line += 1;
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

    drop(tx);
    for code in rx {
        if code != 0 {
            exit_code = code;
        }
    }

    exit_code
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
/// * `tx` Sender for sending the supposed exit code for the program.
fn verify_directory(
    workdir: &PathBuf,
    known_good_path: Arc<String>,
    to_check_path: Arc<String>,
    opts: Arc<super::util::Options>,
    print_line: u32,
    longest_folder: usize,
    myq: Arc<Injector<super::util::HashTask>>,
    tx: Sender<i32>,
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
            myq,
        )
    } else {
        verify_directory_oneshot(&workdir, &opts, &mut failed_paths, myq)
    };

    if success.is_ok() {
        // every file from _algorithm_sum.txt was correct
        inform_directory_good(&workdir, known_good_path, opts);
        tx.send(0).unwrap();
    } else {
        // some files from _algorithm_sum.txt were INCORRECT
        inform_directory_bad(&workdir, to_check_path, opts, &failed_paths);
        tx.send(1).unwrap();
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
    myq: Arc<Injector<super::util::HashTask>>,
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

    let (sender, receiver) = channel();

    for line in BufReader::new(file).lines() {
        if let Ok(line) = line {
            if let Some(captures) = file_path_re.captures(&line) {
                let hash = &captures[1];
                let path = &captures[2];

                let task = super::util::HashTask {
                    path: String::from(path),
                    workdir: PathBuf::from(workdir),
                    opts: Arc::clone(&opts),
                    cmp: String::from(hash),
                    result_chan: sender.clone(),
                };

                myq.push(task);
            }
        }
    }

    drop(sender);

    for task_result in receiver {
        match task_result {
            Ok((mut hashline, cmp)) => {
                hashline.pop();
                if let Some(new_captures) = file_path_re.captures(&hashline) {
                    let new_hash = &new_captures[1];
                    if new_hash != cmp {
                        if opts.loglevel_info() {
                            let now: DateTime<chrono::Local> = chrono::Local::now();
                            println!("[{}] {}: {}", now, workdir.to_str().unwrap(), hashline);
                        }
                        failed_paths.push(String::from(&new_captures[2]));
                        success = false;
                    }
                }
            }
            Err(e) => {
                let now: DateTime<chrono::Local> = chrono::Local::now();
                eprintln!("[{}] {}: {}", now, workdir.to_str().unwrap(), e);

                failed_paths.push(e.to_string());
                success = false;
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
    myq: Arc<Injector<super::util::HashTask>>,
) -> Result<(), io::Error> {
    let mut processed_bytes: u64 = 0;
    let file_path_re = match super::util::regex_from_opts(&opts) {
        Ok(re) => Arc::new(re),
        Err(e) => panic!(e),
    };
    let all_bytes = count_bytes_from_txt(workdir, opts, &file_path_re);
    let workdir_str = workdir.to_str().unwrap();
    let workdir_updater = String::from(workdir_str);
    let file_path_re_updater = Arc::clone(&file_path_re);
    let (tx_result, rx_result): (
        Sender<Result<(String, String), HashError>>,
        Receiver<Result<(String, String), HashError>>,
    ) = channel();
    let (tx_paths, rx_paths) = channel();

    print_progress(
        all_bytes,
        processed_bytes,
        print_line,
        workdir_str,
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

    let updater_handle = std::thread::spawn(move || {
        for task_result in rx_result {
            match task_result {
                Ok((mut hashline, cmp)) => {
                    hashline.pop();
                    if let Some(new_captures) = file_path_re_updater.captures(&hashline) {
                        let new_hash = &new_captures[1];
                        if new_hash != cmp {
                            tx_paths.send(String::from(&new_captures[2])).unwrap();
                        }

                        let metadata =
                            fs::metadata(format!("{}/{}", workdir_updater, &new_captures[2]));
                        if let Ok(metadata) = metadata {
                            processed_bytes += metadata.len();
                        }
                    }

                    print_progress(
                        all_bytes,
                        processed_bytes,
                        print_line,
                        &workdir_updater,
                        longest_folder,
                    )
                    .unwrap();
                }
                Err(e) => {
                    tx_paths.send(e.to_string()).unwrap();
                }
            }
        }
    });

    for line in BufReader::new(file).lines() {
        if let Ok(line) = line {
            if let Some(captures) = file_path_re.captures(&line) {
                let hash = &captures[1];
                let path = &captures[2];

                let task = super::util::HashTask {
                    path: String::from(path),
                    workdir: PathBuf::from(workdir),
                    opts: Arc::clone(&opts),
                    cmp: String::from(hash),
                    result_chan: tx_result.clone(),
                };

                myq.push(task);
            }
        }
    }

    drop(tx_result);

    for path in rx_paths {
        failed_paths.push(path);
    }

    updater_handle.join().unwrap();

    if failed_paths.is_empty() {
        print_message_aligned(print_line, "checked: OK", workdir_str, longest_folder)?;
        Ok(())
    } else {
        print_message_aligned(print_line, "checked: FAILED", workdir_str, longest_folder)?;
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
/// * `workdir` String containing the current working directory, which is printed before the message
/// * `longest_folder` Number of characters in the name of the longest folder, determines how many spaces are padded
fn print_progress(
    all_bytes: u64,
    processed_bytes: u64,
    line: u32,
    workdir: &str,
    longest_folder: usize,
) -> Result<(), io::Error> {
    let progress = processed_bytes as f64 / all_bytes as f64;
    let mut message = format!("{:05.2}% ", progress * 100.0);

    let progress_bar = 60.0 * progress;
    for i in 0..60 {
        if (f64::from(i)) < progress_bar {
            message = format!("{}#", message);
        } else {
            message = format!("{}_", message);
        }
    }

    print_message_aligned(line, &message, workdir, longest_folder)
}

/// Print a message N lines above the current cursor.
/// Cursor position is saved and restored after this operation.
/// The line is cleared before printing.
///
/// # Arguments
/// * `line` Number of lines to scroll up before printing the message
/// * `message` String to print
/// * `workdir` String containing the current working directory, which is printed before the message
fn print_message(line: u32, message: &str, workdir: &str) -> Result<(), io::Error> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(b"\x1b[s")?;
    write!(handle, "\x1b[{}A\x1b[2K", line)?;
    write!(handle, "{}: {}", workdir, message)?;
    handle.write_all(b"\x1b[u")?;
    io::stdout().flush()
}

/// Print a message N lines above the current cursor.
/// Cursor position is saved and restored after this operation.
/// The line is cleared before printing.
/// The message gets padded to the left with spaces in order to align it with
/// other messages on other lines, using longest_folder as an indicator of needed padding.
///
/// # Arguments
/// * `line` Number of lines to scroll up before printing the message
/// * `message` String to print
/// * `workdir` String containing the current working directory, which is printed before the message
/// * `longest_folder` length of the name of the longest folder in the current workset
fn print_message_aligned(
    line: u32,
    message: &str,
    workdir: &str,
    longest_folder: usize,
) -> Result<(), io::Error> {
    let mut padding = String::new();
    let mut i = workdir.len();
    while i < longest_folder {
        padding = format!("{} ", padding);
        i += 1;
    }
    let to_print = &format!("{} {}", padding, message);
    print_message(line, to_print, workdir)
}
