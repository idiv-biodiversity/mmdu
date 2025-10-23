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

use std::path::PathBuf;

use clap::value_parser;
use clap::{Arg, ArgAction, Command};
use clap::{crate_description, crate_name, crate_version};

pub const CONFLICT_FILTER: &str = "the filter options --group and --user are \
                                   in conflict, clap SHOULD NOT allow both to \
                                   be present";

/// Returns command-line parser.
pub fn build() -> Command {
    let dir = Arg::new("dir")
        .value_name("DIR")
        .action(ArgAction::Append)
        .value_parser(is_dir)
        .help("input directories")
        .long_help(
"The input directories for which to gather information. If none are given, \
 directories are read from standard input.",
        );

    let max_depth = Arg::new("max-depth")
        .short('d')
        .long("max-depth")
        .value_name("DEPTH")
        .help("output sub-directory depth")
        .long_help(
"Show the size of each sub-directory up to the given depth including totals \
 for each super-directory. Setting maximum depth to 0 is equivalent to not \
 specifying it at all.",
        )
        .value_parser(value_parser!(usize));

    let count_links = Arg::new("count-links")
        .short('l')
        .long("count-links")
        .action(ArgAction::SetTrue)
        .hide_short_help(true)
        .long_help(
"Count sizes many times if hard linked. The default behavior is to count each \
 hard linked object only once for each point of interest, i.e. once for every \
 DIR and once for every sub-directory if max depth is used."
        );

    let kb_allocated = Arg::new("kb-allocated")
        .long("kb-allocated")
        .action(ArgAction::SetTrue)
        .help("KB_ALLOCATED instead of FILE_SIZE")
        .long_help(
            "Use KB_ALLOCATED instead of FILE_SIZE as the policy attribute.",
        );

    let help = Arg::new("help")
        .short('?')
        .long("help")
        .help("print help (use --help to see all options)")
        .long_help("Print help. Use -? to see a short option listing.")
        .action(ArgAction::Help);

    let version = Arg::new("version")
        .long("version")
        .hide_short_help(true)
        .long_help("Print version.")
        .action(ArgAction::Version);

    Command::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .max_term_width(100)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(dir)
        .args(output_fields())
        .args(filter())
        .args(mmapplypolicy())
        .arg(max_depth)
        .arg(count_links)
        .arg(kb_allocated)
        .arg(help)
        .arg(version)
        .after_help(
"Differences to `du`: `mmdu` defaults to summarized and human readable output \
 and uses apparent size, i.e. `FILE_SIZE` as the policy attribute.",
        )
}

// ----------------------------------------------------------------------------
// argument groups
// ----------------------------------------------------------------------------

fn output_fields() -> Vec<Arg> {
    let block = Arg::new("block")
        .long("block")
        .action(ArgAction::SetTrue)
        .overrides_with_all(["inodes", "both"])
        .help("show only block usage (default)")
        .long_help("Show only block usage. This is the default.")
        .help_heading("Output Fields");

    let inodes = Arg::new("inodes")
        .long("inodes")
        .action(ArgAction::SetTrue)
        .overrides_with_all(["block", "both"])
        .help("show only inode usage")
        .long_help("Show only inode usage.")
        .help_heading("Output Fields");

    let both = Arg::new("both")
        .long("both")
        .action(ArgAction::SetTrue)
        .overrides_with_all(["block", "inodes"])
        .help("show both block usage and inode usage")
        .long_help("Show both block usage and inode usage.")
        .help_heading("Output Fields");

    vec![block, inodes, both]
}

fn filter() -> Vec<Arg> {
    let group = Arg::new("group")
        .long("group")
        .conflicts_with("user")
        .help("filter by group")
        .long_help("Consider only inodes owned by this group.")
        .value_name("name|gid")
        .help_heading("Filtering");

    let user = Arg::new("user")
        .long("user")
        .conflicts_with("group")
        .help("filter by user")
        .long_help("Consider only inodes owned by this user.")
        .value_name("name|uid")
        .help_heading("Filtering");

    vec![group, user]
}

/// Returns arguments forwarded to `mmapplypolicy`.
fn mmapplypolicy() -> Vec<Arg> {
    let nodes = Arg::new("nodes")
        .long("mm-N")
        .hide_short_help(true)
        .long_help(
"Specify list of nodes to use with `mmapplypolicy -N`. For detailed \
 information, see `man mmapplypolicy`.",
        )
        .value_name("all|mount|Node,...|NodeFile|NodeClass")
        .help_heading("Forwarded to `mmapplypolicy`");

    let global_work_dir = Arg::new("global-work-dir")
        .long("mm-g")
        .hide_short_help(true)
        .long_help(
"Specify global work directory to use with `mmapplypolicy -g`. For detailed \
 information, see `man mmapplypolicy`.",
        )
        .value_name("DIR")
        .value_parser(is_dir)
        .help_heading("Forwarded to `mmapplypolicy`");

    let local_work_dir = Arg::new("local-work-dir")
        .long("mm-s")
        .hide_short_help(true)
        .long_help(
"Specify local work directory to use with `mmapplypolicy -s`. Also, the \
 policy LIST output will be written to this directory temporarily before \
 being processed by this tool. Defaults to the system temporary directory. \
 This might be too small for large directories. For detailed information \
 about the `-s` argument, see `man mmapplypolicy`.",
        )
        .value_name("DIR")
        .value_parser(is_dir)
        .help_heading("Forwarded to `mmapplypolicy`");

    vec![nodes, local_work_dir, global_work_dir]
}

// ----------------------------------------------------------------------------
// argument validator
// ----------------------------------------------------------------------------

fn is_dir(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);

    if !path.exists() {
        Err(format!("does not exist: {}", path.display()))
    } else if !path.is_absolute() {
        Err(format!("is not absolute: {}", path.display()))
    } else if path.is_dir() {
        Ok(path)
    } else {
        Err(format!("is not a directory: {}", path.display()))
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    #[test]
    fn verify_cli() {
        super::build().debug_assert();
    }
}
