//! This module implements a filter for a BufReader that filters out filenames
//! that have already been hashed at some point. It does this via reading the _algorithm_sum.txt file.

extern crate regex;

use self::regex::Regex;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read};


/// The structure that gets wrapped around a BufReader to filter it
pub struct Filter<T> {
    /// The filenames that were already hashed in the past, gathered through reading _algorithm_sum.txt
    already_calculated_files: HashMap<String, bool>,
    /// The BufReader that will be read and filtered
    input: BufReader<T>,
    /// The algorithm that was used to hash the files eg "sha1"
    algorithm: String
}

impl<T> Filter<T> {
    /// Creates a new instance of Filter
    ///
    /// # Arguments
    ///
    /// * `input` The BufReader which lines will be filtered through this object
    /// * `sumfile_path` The path to the _algorithm_sum.txt file that contains the already calculated hashsums
    /// * `opts` A reference to the Options object containing information about the program behavior
    ///
    /// # Errors
    ///
    /// If the _algorithm_sum.txt file can not be read or the algorithm can not be recognized,
    /// an Err will be returned instead of a Filter.
    pub fn new(input: BufReader<T>, sumfile_path: &str, opts: &super::util::Options) -> Result<Filter<T>, &'static str> {
        let mut already_calculated_files = HashMap::new();

        match OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(format!("{}/{}sum.txt", sumfile_path, opts.algorithm)) {

            Err(_) => {
                return Err("Could not open _algorithm_sum.txt");
            },

            Ok(file) => {
                let file_path_re = match opts.algorithm.as_ref() {
                    "sha1" => Regex::new(r"[[:xdigit:]]{40}\s\s(.*)$").unwrap(),
                    "md5" => Regex::new(r"[[:xdigit:]]{32}\s\s(.*)$").unwrap(),
                    "sha224" => Regex::new(r"[[:xdigit:]]{56}\s\s(.*)$").unwrap(),
                    "sha256" => Regex::new(r"[[:xdigit:]]{64}\s\s(.*)$").unwrap(),
                    "sha384" => Regex::new(r"[[:xdigit:]]{96}\s\s(.*)$").unwrap(),
                    "sha512" => Regex::new(r"[[:xdigit:]]{128}\s\s(.*)$").unwrap(),
                    _ => {return Err("Could not recognize hashing algorithm")}
                };

                for line in BufReader::new(file).lines() {
                    if let Ok(line) = line {
                        if let Some(captures) = file_path_re.captures(&line) {
                            let path = &captures[1];
                            already_calculated_files.insert(path.to_string(), true);
                        } else { continue }
                    } else { continue }
                }

                Ok(Filter{already_calculated_files, input, algorithm: opts.algorithm.clone()})
            }
        }
    }
}

impl<T: Read> Iterator for Filter<T> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        for line in self.input.by_ref().lines() {
            match line {
                Err(_) => continue,
                Ok(line) => {
                    let contained = self.already_calculated_files.contains_key(&line);
                    if contained {
                        continue
                    }

                    if line == format!("./{}sum.txt", self.algorithm) {
                        continue
                    }

                    return Some(line)
                }
            }
        }

        return None
    }
}
