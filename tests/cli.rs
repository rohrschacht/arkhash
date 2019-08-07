extern crate assert_cli;
extern crate regex;
// extern crate assert_cmd;
// extern crate predicates;

use assert_cli::*;
use regex::Regex;
// use assert_cmd::prelude::*;
// use predicates::prelude::*;
// use std::process::Command;

use std::fs;
use std::io::prelude::*;
use std::io::{BufRead, BufReader};

use std::sync::Mutex;
#[macro_use]
extern crate lazy_static;
lazy_static! {
    static ref MTX: Mutex<()> = Mutex::new(());
}

#[test]
#[ignore]
fn template_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test

    teardown();
}

#[test]
fn update_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test
    Assert::main_binary()
        .with_args(&["-u"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 27 {
            teardown();
            panic!(
                "hashfile does not contain enough lines. expected: 27, given: {}",
                i
            );
        }
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    Assert::main_binary()
        .with_args(&["-u"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 27 {
            teardown();
            panic!(
                "arkhash added new lines to the hashfile on update. expected: 27, given: {}",
                i
            );
        }
    }

    let mut f = fs::File::create("testenvironment/update_test").unwrap();
    f.write_all(b"Test").unwrap();

    Assert::main_binary()
        .with_args(&["-u"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 28 {
            teardown();
            panic!(
                "arkhash did not detect a new file. expected: 28, given: {}",
                i
            );
        }
    }

    teardown();
}

#[test]
fn verify_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test
    Assert::main_binary()
        .with_args(&["-u"])
        .current_dir("testenvironment")
        .unwrap();

    Assert::main_binary()
        .with_args(&["-v"])
        .current_dir("testenvironment")
        .stdout()
        .doesnt_contain("FAILED")
        .unwrap();

    teardown();
}

#[test]
fn verify_modified_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test
    Assert::main_binary()
        .with_args(&["-u"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/sha1sum.txt");
    let mut data = String::new();
    if let Ok(mut hashfile) = hashfile {
        hashfile.read_to_string(&mut data).unwrap();
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    let mut first = true;
    let mut modified = String::new();
    for line in data.split("\n") {
        if first {
            let mut modline = String::from(line);
            modline.remove(0);
            modline.insert(0, '0');
            modified.push_str(&format!("{}\n", modline));
            first = false;
        } else {
            modified.push_str(&format!("{}\n", line));
        }
    }

    let mut hashfile = fs::File::create("testenvironment/sha1sum.txt").unwrap();
    hashfile.write(modified.as_bytes()).unwrap();

    Assert::main_binary()
        .with_args(&["-v"])
        .current_dir("testenvironment")
        .stdout()
        .contains("FAILED")
        .unwrap();

    teardown();
}

#[test]
fn update_subdir_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test
    Assert::main_binary()
        .with_args(&["-us"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/test/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 13 {
            teardown();
            panic!(
                "hashfile does not contain enough lines. expected: 13, given: {}",
                i
            );
        }
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    let hashfile = fs::File::open("testenvironment/secondsecond/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 14 {
            teardown();
            panic!(
                "hashfile does not contain enough lines. expected: 14, given: {}",
                i
            );
        }
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    Assert::main_binary()
        .with_args(&["-us"])
        .current_dir("testenvironment")
        .unwrap();

    let hashfile = fs::File::open("testenvironment/test/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 13 {
            teardown();
            panic!(
                "arkhash added new lines to the hashfile on update. expected: 13, given: {}",
                i
            );
        }
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    let hashfile = fs::File::open("testenvironment/secondsecond/sha1sum.txt");
    if let Ok(hashfile) = hashfile {
        let mut i = 0;
        for _ in BufReader::new(hashfile).lines() {
            i += 1;
        }

        if i != 14 {
            teardown();
            panic!(
                "arkhash added new lines to the hashfile on update. expected: 14, given: {}",
                i
            );
        }
    } else {
        teardown();
        panic!("arkhash did not create the hashfile!");
    }

    teardown();
}

#[test]
fn verify_subdir_test() {
    let _guard = MTX.lock().unwrap();

    setup();

    // test
    Assert::main_binary()
        .with_args(&["-us"])
        .current_dir("testenvironment")
        .unwrap();

    Assert::main_binary()
        .with_args(&["-vs"])
        .current_dir("testenvironment")
        .stdout()
        .doesnt_contain("FAILED")
        .unwrap();

    let mut known_good_found = false;
    let re = Regex::new(r"known_good.*").unwrap();
    for entry in fs::read_dir("testenvironment").unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            if re.is_match(path.to_str().unwrap()) {
                known_good_found = true;
            }
        }
    }

    assert!(known_good_found);

    teardown();
}

fn setup() {
    fs::create_dir("testenvironment").unwrap();
    fs::create_dir("testenvironment/test").unwrap();
    fs::create_dir("testenvironment/secondsecond").unwrap();

    for i in 1..10 {
        let mut f1 = fs::File::create(format!("testenvironment/test/little_{}", i)).unwrap();
        let mut f2 =
            fs::File::create(format!("testenvironment/secondsecond/little_{}", i)).unwrap();

        f1.write_all(b"Small file").unwrap();
        f2.write_all(b"Small file").unwrap();
    }

    for i in 1..5 {
        let mut f1 = fs::File::create(format!("testenvironment/test/middle_{}", i)).unwrap();
        let mut f2 =
            fs::File::create(format!("testenvironment/secondsecond/middle_{}", i)).unwrap();

        f1.write_all(b"Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet.").unwrap();
        f2.write_all(b"Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet.").unwrap();
    }

    let mut f = fs::File::create("testenvironment/secondsecond/big_1").unwrap();
    f.write_all(b"Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet.

Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis at vero eros et accumsan et iusto odio dignissim qui blandit praesent luptatum zzril delenit augue duis dolore te feugait nulla facilisi. Lorem ipsum dolor sit amet, consectetuer adipiscing elit, sed diam nonummy nibh euismod tincidunt ut laoreet dolore magna aliquam erat volutpat.

Ut wisi enim ad minim veniam, quis nostrud exerci tation ullamcorper suscipit lobortis nisl ut aliquip ex ea commodo consequat. Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis at vero eros et accumsan et iusto odio dignissim qui blandit praesent luptatum zzril delenit augue duis dolore te feugait nulla facilisi.

Nam liber tempor cum soluta nobis eleifend option congue nihil imperdiet doming id quod mazim placerat facer possim assum. Lorem ipsum dolor sit amet, consectetuer adipiscing elit, sed diam nonummy nibh euismod tincidunt ut laoreet dolore magna aliquam erat volutpat. Ut wisi enim ad minim veniam, quis nostrud exerci tation ullamcorper suscipit lobortis nisl ut aliquip ex ea commodo consequat.

Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis.

At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, At accusam aliquyam diam diam dolore dolores duo eirmod eos erat, et nonumy sed tempor et et invidunt justo labore Stet clita ea et gubergren, kasd magna no rebum. sanctus sea sed takimata ut vero voluptua. est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat.

Consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus.

Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est Lorem ipsum dolor sit amet.

Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis at vero eros et accumsan et iusto odio dignissim qui blandit praesent luptatum zzril delenit augue duis dolore te feugait nulla facilisi. Lorem ipsum dolor sit amet, consectetuer adipiscing elit, sed diam nonummy nibh euismod tincidunt ut laoreet dolore magna aliquam erat volutpat.

Ut wisi enim ad minim veniam, quis nostrud exerci tation ullamcorper suscipit lobortis nisl ut aliquip ex ea commodo consequat. Duis autem vel eum iriure dolor in hendrerit in vulputate velit esse molestie consequat, vel illum dolore eu feugiat nulla facilisis at vero eros et accumsan et iusto odio dignissim qui blandit praesent luptatum zzril delenit augue duis dolore te feugait nulla facilisi.

Nam liber tempor cum soluta nobis eleifend option congue nihil imperdiet doming id quod mazim placerat facer possim assum. Lorem ipsum dolor sit amet, consectetuer adipiscing elit, sed diam nonummy nibh euismod tincidunt ut laoreet dolore magna aliquam erat volutpat. Ut wisi enim ad minim veniam, quis nostrud exerci tation ullamcorper suscipit lobortis nisl ut aliquip ex ea commodo").unwrap();
}

fn teardown() {
    fs::remove_dir_all("testenvironment").unwrap();
}
