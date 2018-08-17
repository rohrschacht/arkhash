pub mod util;
pub mod filter;
pub mod update;
pub mod verify;


fn main() {
    let opts = util::Options::new(std::env::args().collect());

    if opts.help {
        const VERSION: &'static str = env!("CARGO_PKG_VERSION");
        print!("{} Version {}\n\n\
        "
               , opts.program_name, VERSION);
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
