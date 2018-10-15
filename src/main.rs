pub mod util;
pub mod filter;
pub mod update;
pub mod verify;


fn main() {
    let opts = util::Options::new(std::env::args().collect());

    if opts.help {
        const VERSION: &'static str = env!("CARGO_PKG_VERSION");
        print!("{} Version {}

Usage:
 {} [OPTION] [DIRECTORY]

Arguments:
 -a, --algo, --algorithm ALGORITHM      uses ALGORITHM to hash files (example: md5, default: sha1)
 -s, --subdir, --subdirectories         operate on the subdirectories of DIRECTORY (only for update and verify mode)
 --loglevel LEVEL                       controls the output of the program (quiet/info/progress/debug)
                                        progress currently only supported for verify mode
 --quiet                                sets the loglevel to quiet
 -T, --threads THREADS                  spawn a maximum of THREADS worker threads (default: 0: no cap)
 -h, --help                             show this help message
 -u, --update                           switch to update mode
 -v, --verify                           switch to verify mode
"
               , opts.program_name, VERSION, opts.program_name);
        return;
    }

    if opts.loglevel_debug() {
        println!("{:?}", opts);
    }

    match opts.mode {
        util::Mode::Filter => {
            let reader = std::io::BufReader::new(std::io::stdin());
            let filter = filter::Filter::new(reader, opts.folder.as_str(), &opts);

            match filter {
                Err(e) => panic!(e),
                Ok(filter) => {
                    for line in filter {
                        println!("{}", line);
                    }
                }
            }
        },
        util::Mode::Update => {
            update::update_directories(opts);
        },
        util::Mode::Verify => {
            verify::verify_directories(opts);
        }
    }
}
