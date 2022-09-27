/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019-2022  Christian Krause                              *
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

#![deny(clippy::all)]
#![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]

mod cli;
mod config;
mod log;
mod output;
mod policy;
mod usage;

use std::io;
use std::path::Path;

use atty::Stream;

use crate::config::Config;

fn main() {
    let config = config::get();
    log::debug(format!("{:#?}", config), &config);

    // ALLOW if let is easier to comprehend
    #[allow(clippy::option_if_let_else)]
    if let Some(dirs) = &config.dirs {
        for dir in dirs {
            run(dir, &config);
        }
    } else {
        let interactive = atty::is(Stream::Stdin);

        if interactive {
            log::warning("input is read from terminal");
            log::warning("only experts do this on purpose");
            log::warning("you may have forgotten to either");
            log::warning("- specify directories on the command line or");
            log::warning("- pipe data into this tool");
            log::warning("press CTRL-D or CTRL-C to exit");
        }

        let lines = io::stdin().lines();
        for line in lines {
            let dir = line.unwrap();
            run(Path::new(&dir), &config);
        }
    }
}

fn run(dir: &Path, config: &Config) {
    log::debug(format!("running {} ...", dir.display()), config);
    if let Err(error) = usage::run(dir, config) {
        log::error(format!("skipping directory {}: {}", dir.display(), error));
    }
}
