# arkhash

## About
This program was designed based on the need of ensuring data integrity of non-changing, archived data.

Let's take a look at a picture archive for example.       
The idea is that the hashsum of a file should not change if you don't deliberately alter the file.
Once you sorted the pictures into the correct directory structure they are not expected to change.
The hashsum of a picture will stay the same. But it will change if for example some sectors of your
hard drive containing the picture rot.

By generating a hashsum when the picture is initially stored on the drive and comparing it regularly,
this program is able to detect those unwanted changes. It can't repair the picture, but it can inform
you. You are then able to retrieve the picture from a backup, which you should always have when dealing
with sensible data.

## Features
* Supported algorithms: sha1, md5, sha224, sha256, sha384, sha512 (default: sha1)
* Update the hashsums of a directorys content, thereby not recalculating previously calculated files
* Verify the hashsums of a directorys content
* Filter out paths to files that have been hashed before
* Multiple loglevels to control verbosity
* Use multiple threads to increase performance
* Show progress in verify mode with progress bars
* Ignore directories from .hfignore in subdir-mode

## Dependencies
arkhash depends on sha1sum, md5sum, etc. to calculate the hashes.
These should already be installed on your gnu/linux system.

## Usage
The program has three major modes.        
The modes will be briefly described. It is assumed that the default sha1 algorithm is used for hashing,
but it works the same for every other algorithm.

### Filter Mode
This is the main mode of the program. Unless specified via command line options the program will run
in this mode.

On startup, the program will search for a sha1sum.txt in the current directory. It will read that
file and detect all paths from every line. It will then read STDIN and only print out those paths
to STDOUT that have not been found in the sha1sum file. This is the core component needed for the
update mode, exposed here to you if you want to do something else with the unhashed files.

Example usage:        
```
find . | arkhash | xargs -i -d'\n' sha1sum {}
```

### Update Mode
The program will hash every file in the current directory and every subdirectory recursively and store
the hashes in a sha1sum.txt file. It won't calculate any hashes for files that are listed in the 
sha1sum.txt. An update on a directory where no new files were added is a quick operation. If a file
should be rehashed, the coresponding line in sha1sum.txt can be deleted and the file will be rehashed
on the next update.

### Verify Mode
The program will check if the files listed in sha1sum.txt have not changed. It does this by invoking
```
sha1sum -c --quiet sha1sum.txt
```
If the check of a file has failed you will be immediately informed via STDOUT and the path to the
file will be stored in a to_check.txt file.

Progressbars can be activated by using the progress loglevel.
They also work in subdir mode.
```
arkhash -v --loglevel=progress
arkhash -vs --loglevel=progress
```

### Subdir Mode
Let's assume you order your pictures like this:
```
pictures
├── 2015
├── 2016
├── 2017
└── 2018
```
If you enable the subdir mode for the update or verify mode, arkhash will start 4 threads and
calculate the hashes in every subdirectory of pictures, or verify them respectively.
The sha1sum.txt files will be stored in the subdirectories 2015, 2016, etc. .
This should increase throughput and decrease execution time on multi-core machines.
You can limit the number of threads arkhash will spawn via command line options.
If you limit the number of threads to 2 in this example, arkhash will operate on 2015 and 2016
concurrently and go on with the next folder if one of them finishes.

#### .hfignore File
When the program operates in Update-Subdir mode, it will read a .hfignore text file in the working directory if it exists.
You can specify subdirectories that should be ignored by this program. Just list the names of these
directories line by line.

In the following example the directories "editing-workspace" and "trash" will be ignored on updating
(and thereby on verifying).
```
.hfignore contents:
editing-workspace
trash

filesystem:
pictures
├── 2015
├── 2016
├── 2017
├── 2018
├── editing-workspace
└── trash
```

## Help message
```
Usage:
 arkhash [OPTION] [DIRECTORY]

Arguments:
 -a, --algo, --algorithm ALGORITHM      uses ALGORITHM to hash files (example: md5, default: sha1)
 -s, --subdir, --subdirectories         operate on the subdirectories of DIRECTORY (only for update and verify mode)
 --loglevel LEVEL                       controls the output of the program (quiet/info/debug)
 --quiet                                sets the loglevel to quiet
 -T, --threads THREADS                  spawn a maximum of THREADS worker threads (default: 0: no cap)
 -h, --help                             show this help message
 -u, --update                           switch to update mode
 -v, --verify                           switch to verify mode
```

## Planned features
* Operating on a directory given on the command line
 * Currently arkhash is mainly implemented and only tested to work on the current working directory

## Acknowledgements
This implementation of arkhash makes use of the following awesome open source projects:
* [Rust programming language](https://www.rust-lang.org)
* [regex](https://crates.io/crates/regex)
* [chrono](https://crates.io/crates/chrono)
* [threadpool](https://crates.io/crates/threadpool)
* [termios](https://crates.io/crates/termios)
