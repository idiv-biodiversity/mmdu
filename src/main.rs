/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019  Christian Krause                                   *
 *                                                                           *
 *  Christian Krause  <christian.krause@idiv.de>                             *
 *                                                                           *
 * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  This file is part of mmdu.                                               *
 *                                                                           *
 *  mmdu is free software: you can redistribute it and/or modify             *
 *  it under the terms of the GNU General Public License as published by     *
 *  the Free Software Foundation, either version 3 of the license, or any    *
 *  later version.                                                           *
 *                                                                           *
 *  mmdu is distributed in the hope that it will be useful, but              *
 *  WITHOUT ANY WARRANTY; without even the implied warranty of               *
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU         *
 *  General Public License for more details.                                 *
 *                                                                           *
 *  You should have received a copy of the GNU General Public License along  *
 *  with mmdu. If not, see <http://www.gnu.org/licenses/>.                   *
 *                                                                           *
 * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * */


extern crate atty;
extern crate bytesize;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
extern crate mktemp;
extern crate regex;

mod app;
mod config;
mod log;
mod output;
mod policy;
mod usage;

use atty::Stream;
use std::io::{self, BufRead};

use crate::config::Config;

fn main() {
    let args = app::args();

    let config = config::Config::from_args(&args);

    match args.values_of("dir") {
        Some(dirs) => {
            for dir in dirs {
                run(dir, &config);
            }
        },

        None => {
            let interactive = atty::is(Stream::Stdin);

            if interactive {
                log::warning("input is read from terminal");
                log::warning("only experts do this on purpose");
                log::warning("you may have forgotten to either");
                log::warning("- specify directories on the command line or");
                log::warning("- pipe data into this tool");
                log::warning("press CTRL-D or CTRL-C to exit");
            }

            let stdin = io::stdin();

            for line in stdin.lock().lines() {
                let dir = &line.unwrap();
                run(dir, &config);
            }
        },
    }
}

fn run(dir: &str, config: &Config) {
    let result = usage::run(dir, config);

    match result {
        Err(error) => {
            log::error(
                &format!("skipping directory {}: {}", dir, error)
            );
        }

        Ok(()) => (),
    }
}
