/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019-2023  Christian Krause                              *
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

use clap::ArgMatches;

pub fn get() -> Config {
    let cli = crate::cli::build();
    let args = cli.get_matches();
    Config::from(args)
}

#[derive(Debug)]
pub struct Config {
    pub dirs: Option<Vec<PathBuf>>,
    pub debug: bool,
    pub filter: Option<Filter>,
    pub max_depth: Option<usize>,
    pub mm_nodes: Option<String>,
    pub mm_local_work_dir: Option<PathBuf>,
    pub mm_global_work_dir: Option<PathBuf>,
    pub count_bytes: bool,
    pub count_inodes: bool,
}

impl From<ArgMatches> for Config {
    fn from(args: ArgMatches) -> Self {
        let dirs = args
            .get_many::<PathBuf>("dir")
            .map(|x| x.map(ToOwned::to_owned).collect::<Vec<_>>());

        let debug = args.get_one::<bool>("debug").copied().unwrap_or_default();

        let group = args.get_one::<String>("group");
        let user = args.get_one::<String>("user");

        let filter = match (group, user) {
            (None, None) => None,
            (Some(group), None) => Some(Filter::Group(group.clone())),
            (None, Some(user)) => Some(Filter::User(user.clone())),
            (Some(_), Some(_)) => {
                unreachable!("{}", crate::cli::CONFLICT_FILTER)
            }
        };

        let max_depth = args
            .get_one::<usize>("max-depth")
            .copied()
            .filter(|depth| *depth > 0);

        let mm_nodes = args.get_one::<String>("nodes").cloned();

        let mm_local_work_dir =
            args.get_one::<PathBuf>("local-work-dir").cloned();

        let mm_global_work_dir =
            args.get_one::<PathBuf>("global-work-dir").cloned();

        let (count_bytes, count_inodes) = cbi(&args);

        Self {
            dirs,
            debug,
            filter,
            max_depth,
            mm_nodes,
            mm_local_work_dir,
            mm_global_work_dir,
            count_bytes,
            count_inodes,
        }
    }
}

#[derive(Debug)]
pub enum Filter {
    Group(String),
    User(String),
}

/// Returns whether to count block usage and/or inode usage.
fn cbi(args: &ArgMatches) -> (bool, bool) {
    let block = args.get_flag("block");
    let inodes = args.get_flag("inodes");
    let both = args.get_flag("both");

    // most conditions aren't possible due to `overrides_with_all`
    match (block, inodes, both) {
        (_, true, _) => (false, true),
        (_, _, true) => (true, true),
        _ => (true, false),
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    #[test]
    fn cbi() {
        let cli = crate::cli::build();

        assert_eq!(
            (true, false),
            super::cbi(&cli.clone().get_matches_from([clap::crate_name!()]))
        );

        assert_eq!(
            (true, false),
            super::cbi(
                &cli.clone()
                    .get_matches_from([clap::crate_name!(), "--block"])
            )
        );

        assert_eq!(
            (false, true),
            super::cbi(
                &cli.clone()
                    .get_matches_from([clap::crate_name!(), "--inodes"])
            )
        );

        assert_eq!(
            (true, true),
            super::cbi(
                &cli.clone()
                    .get_matches_from([clap::crate_name!(), "--both"])
            )
        );

        assert_eq!(
            (false, true),
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--block",
                "--inodes"
            ]))
        );

        assert_eq!(
            (true, true),
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--block",
                "--both"
            ]))
        );

        assert_eq!(
            (true, false),
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--inodes",
                "--block"
            ]))
        );

        assert_eq!(
            (true, true),
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--inodes",
                "--both"
            ]))
        );

        assert_eq!(
            (true, false),
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--both",
                "--block"
            ]))
        );

        assert_eq!(
            (false, true),
            super::cbi(&cli.get_matches_from([
                clap::crate_name!(),
                "--both",
                "--inodes"
            ]))
        );
    }
}
