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

use atty::Stream;
use clap::{crate_description, crate_name, crate_version};
use clap::{App, AppSettings, Arg, ArgMatches};
use std::path::Path;

pub fn args() -> ArgMatches<'static> {
    let color = atty::is(Stream::Stdout);

    let color = if color {
        AppSettings::ColoredHelp
    } else {
        AppSettings::ColorNever
    };

    let dir = Arg::with_name("dir")
        .help("input directories")
        .long_help(
"The input directories for which to gather information. If none are given, \
 directories are read from standard input.",
        )
        .multiple(true)
        .validator(is_dir);

    let debug = Arg::with_name("debug")
        .hidden_short_help(true)
        .long("debug")
        .help("debug output")
        .display_order(2);

    let max_depth = Arg::with_name("max-depth")
        .short("d")
        .long("max-depth")
        .takes_value(true)
        .value_name("DEPTH")
        .help("output sub-directory depth")
        .long_help(
"Show the size of each sub-directory up to the given depth including totals \
 for each super-directory. Setting maximum depth to 0 is equivalent to not \
 specifying it at all.",
        )
        .display_order(1)
        .validator(is_number);

    let nodes = Arg::with_name("nodes")
        .short("N")
        .help("use for mmapplypolicy -N argument")
        .long_help(
"Specify list of nodes to use with `mmapplypolicy -N`. For detailed \
 information, see `man mmapplypolicy`.",
        )
        .value_name("all|mount|Node,...|NodeFile|NodeClass")
        .takes_value(true)
        .display_order(2);

    let global_working_dir = Arg::with_name("global-working-dir")
        .short("g")
        .help("use for mmapplypolicy -g argument")
        .long_help(
"Specify global work directory to use with `mmapplypolicy -g`. For detailed \
 information, see `man mmapplypolicy`.",
        )
        .takes_value(true)
        .value_name("dir")
        .display_order(3)
        .validator(is_dir);

    let local_working_dir = Arg::with_name("local-working-dir")
        .short("s")
        .help("use for mmapplypolicy -s argument and policy output")
        .long_help(
"Specify local work directory to use with `mmapplypolicy -s`. Also, the \
 policy LIST output will be written to this directory temporarily before \
 being processed by this tool. Defaults to the system temporary directory. \
 This might be too small for large directories. For detailed information \
 about the `-s` argument, see `man mmapplypolicy`.",
        )
        .takes_value(true)
        .value_name("dir")
        .display_order(3)
        .validator(is_dir);

    App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .global_setting(color)
        .max_term_width(80)
        .help_short("?")
        .help_message("show this help output")
        .version_message("show version")
        .arg(dir)
        .arg(debug)
        .arg(max_depth)
        .arg(nodes)
        .arg(local_working_dir)
        .arg(global_working_dir)
        .after_help(
"Differences to du: mmdu defaults to summarized and human readable output and \
 uses apparent size.",
        )
        .get_matches()
}

#[allow(clippy::needless_pass_by_value)]
fn is_dir(s: String) -> Result<(), String> {
    let path = Path::new(&s);

    if !path.exists() {
        Err(format!("does not exist: {:?}", path))
    } else if !path.is_absolute() {
        Err(format!("is not absolute: {:?}", path))
    } else if path.is_dir() {
        Ok(())
    } else {
        Err(format!("is not a directory: {:?}", path))
    }
}

#[allow(clippy::needless_pass_by_value)]
fn is_number(s: String) -> Result<(), String> {
    if s.parse::<usize>().is_ok() {
        Ok(())
    } else {
        Err(format!("not a positive number: {}", s))
    }
}
