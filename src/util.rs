//! This module describes a set of utilities that will be used throughout the other modules

/// The mode the program will operate in
#[derive(Debug, Clone)]
pub enum Mode {
    Filter,
    Update,
    Verify
}

/// The level of detail the program will be logging
#[derive(Debug, PartialEq, Clone)]
pub enum LogLevel {
    Quiet,
    Info,
    Debug
}

/// A single structure that gets constructed by commandline arguments and describes the behavior of the program
#[derive(Debug, Clone)]
pub struct Options {
    /// Whether or not the help message will be displayed
    pub help: bool,
    /// My name
    pub program_name: String,
    /// The hashing algorithm to use
    pub algorithm: String,
    /// Whether or not it will be operated on a single folder or every subfolder
    pub subdir_mode: bool,
    /// The mode the program will operate in
    pub mode: Mode,
    /// The level of detail the program will be logging
    pub log_level: LogLevel,
    /// Maximum number of threads to spawn
    pub num_threads: usize,
    /// The folder to operate on
    pub folder: String
}

impl Options {
    /// Creates a new instance of Options containing all settings given through the commandline
    ///
    /// # Arguments
    ///
    /// * `args` - A vec of Strings containing all commandline parameters
    pub fn new(args: Vec<String>) -> Options {
        let mut opts = Options {
            help: false,
            program_name: "hashfilter".to_string(),
            algorithm: "sha1".to_string(),
            subdir_mode: false,
            mode: Mode::Filter,
            log_level: LogLevel::Info,
            num_threads: 0,
            folder: ".".to_string()
        };

        // prepare Strings for parsing
        let args = prepare_args(args);

        opts.program_name = args[0].clone();

        // loop through every argument, except the name
        for i in 1..args.len() {
            let arg = &args[i];

            // match options (Strings with leading -)
            if arg.starts_with("-") {
                match arg.as_ref() {
                    "-a" | "--algo" | "--algorithm" => opts.algorithm = args[i + 1].clone().to_lowercase(),
                    "-s" | "--subdir" | "--subdirs" | "--subdirectories" => opts.subdir_mode = true,
                    "-u" | "--update" => opts.mode = Mode::Update,
                    "-v" | "--verify" => opts.mode = Mode::Verify,
                    "--loglevel" | "--log_level" | "--log-level" => opts.log_level = {
                        match args.get(i + 1).expect(format!("Usage: {} {} quiet/info/debug", opts.program_name, args[i]).as_ref()).as_ref() {
                            "none" | "quiet" | "0" => LogLevel::Quiet,
                            "info" | "1" => LogLevel::Info,
                            "debug" | "2" => LogLevel::Debug,
                            _ => LogLevel::Info
                        }
                    },
                    "--quiet" => opts.log_level = LogLevel::Quiet,
                    "-T" | "--threads" => opts.num_threads = args.get(i + 1).expect(format!("Usage: {} -T NUMBER_OF_MAX_THREADS", opts.program_name).as_ref())
                        .trim().parse().expect(format!("Usage: {} -T NUMBER_OF_MAX_THREADS", opts.program_name).as_ref()),
                    "-h" | "--help" => opts.help = true,
                    _ => opts.help = true
                }
            } else {
                // if a String does not start with - and the String before it is none of the below, it is the folder to operate on
                match args[i - 1].as_ref() {
                    "--loglevel" | "--log_level" | "--log-level" | "-a" | "--algo" | "--algorithm" | "-T" | "--threads" => {},
                    _ => opts.folder = arg.clone()
                }
            }
        }

        opts
    }

    /// Indicates that the program is in the debug loglevel
    pub fn loglevel_debug(&self) -> bool {
        self.log_level == LogLevel::Debug
    }

    /// Indicates that the program is at least in the info loglevel
    pub fn loglevel_info(&self) -> bool {
        self.log_level == LogLevel::Debug || self.log_level == LogLevel::Info
    }
}

/// Prepares a vec of Strings for parsing options
///
/// A new vec gets returned that contains more Strings than the original, because two rules get applied:
/// * If a String starts with a single -, but it has more than 2 characters, the parameters get split
///   into single Strings with a leading -
/// * If a String contains a =, the = will get cut and the prefix and suffix will be split into two Strings
/// This is necessary for the match statement in Options::new to work correctly
///
/// # Arguments
///
/// * `args` A vec of Strings containing all commandline parameters
fn prepare_args(args: Vec<String>) -> Vec<String> {
    let mut prepared_args = Vec::with_capacity(args.len());

    for arg in args {
        match arg.contains("=") {
            false => {
                if arg.contains("-") && !arg.contains("--") && arg.len() > 2 {
                    let characters = &arg[1..];
                    for char in characters.chars() {
                        let single_arg = format!("-{}", char);
                        prepared_args.push(single_arg);
                    }
                } else {
                    prepared_args.push(arg);
                }
            },
            true => {
                let position = arg.find("=").unwrap();
                let prefix = arg[0..position].to_string();
                let suffix = arg[position + 1 ..].to_string();
                prepared_args.push(prefix);
                prepared_args.push(suffix);
            }
        }


    }

    prepared_args
}