/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019-2024  Christian Krause                              *
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
// ALLOW only windows deps ATM, we don't use these targets
#![allow(clippy::multiple_crate_versions)]

mod cli;
mod config;
mod output;
mod policy;
mod usage;

use std::path::{Path, PathBuf};

use anyhow::Result;
use args_or_stdin::ValuesRefExt;
use clap::crate_name;

use crate::config::Config;

fn main() -> Result<()> {
    #[cfg(feature = "log")]
    env_logger::init();

    let cli = cli::build();
    let args = cli.get_matches();
    let config = Config::try_from(&args)?;

    #[cfg(feature = "log")]
    log::debug!("{config:#?}");

    let dirs = args.get_many::<String>("dir");

    for dir in dirs.or_stdin(crate_name!()) {
        let dir = dir?;
        let dir = PathBuf::from(dir.into_owned());
        run(&dir, &config);
    }

    Ok(())
}

fn run(dir: &Path, config: &Config) {
    #[cfg(feature = "log")]
    log::debug!("running with directory {} ...", dir.display());

    if let Err(error) = usage::run(dir, config) {
        let dir = dir.display();

        #[cfg(not(feature = "log"))]
        eprintln!("{}: skipping {dir}: {error:?}", clap::crate_name!());

        #[cfg(feature = "log")]
        log::warn!("skipping directory {dir}: {error:#}");
    }
}
