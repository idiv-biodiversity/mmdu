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


use atty::Stream;
use clap::{App, AppSettings, Arg, ArgMatches};
use regex::Regex;
use std::path::Path;

pub fn args() -> ArgMatches<'static> {
    let color = atty::is(Stream::Stdout);

    let color = if color {
        AppSettings::ColoredHelp
    } else {
        AppSettings::ColorNever
    };

    App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .global_setting(color)
        .max_term_width(80)
        .help_short("?")
        .help_message("show this help output")
        .version_message("show version")
        .arg(Arg::with_name("dir")
             .help("input directories")
             .long_help("The input directories for which to gather \
                         information. If none are given, directories are read \
                         from standard input.")
             .validator(is_dir)
             .multiple(true))
        .arg(Arg::with_name("debug")
             .hidden(true)
             .long("debug")
             .help("debug output")
             .display_order(2))
        .arg(Arg::with_name("max-depth")
             .short("d")
             .long("max-depth")
             .takes_value(true)
             .value_name("DEPTH")
             .help("output sub-directory depth")
             .long_help("Show the size of each sub-directory up to the given \
                         depth including totals for each super-directory. \
                         Setting maximum depth to 0 is equivalent to not \
                         specifying it at all.")
             .validator(unsigned_integer)
             .display_order(1))
        .arg(Arg::with_name("nodes")
             .short("N")
             .long("nodes")
             .value_name("all|mount|Node,...|NodeFile|NodeClass")
             .help("worker nodes")
             .long_help("Specify the worker nodes. The argument is forwarded \
                         to mmapplypolicy, see man mmapplypolicy for more \
                         information.")
             .takes_value(true))
        .after_help("Differences to du: mmdu defaults to summarized and human \
                     readable output and uses apparent size.")
        .get_matches()
}

fn is_dir(s: String) -> Result<(), String> {
    let path = Path::new(&s);

    if !path.exists() {
        Err(format!("does not exist: {:?}", path))
    } else if !path.is_absolute() {
        Err(format!("is not absolute: {:?}", path))
    } else if !path.is_dir() {
        Err(format!("is not a directory: {:?}", path))
    } else {
        Ok(())
    }
}

fn unsigned_integer(s: String) -> Result<(), String> {
    lazy_static! {
        static ref NUMBER_RE: Regex = Regex::new(
            r#"^\d+$"#
        ).unwrap();
    }

    if NUMBER_RE.is_match(&s) {
        Ok(())
    } else {
        Err(format!("not a number: {}", s))
    }
}
