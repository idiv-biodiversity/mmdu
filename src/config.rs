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
    pub max_depth: Option<usize>,
    pub mm_nodes: Option<String>,
    pub mm_local_work_dir: Option<String>,
    pub mm_global_work_dir: Option<String>,
    pub count_bytes: bool,
    pub count_inodes: bool,
}

impl From<ArgMatches> for Config {
    fn from(args: ArgMatches) -> Self {
        let dirs = args
            .get_many::<PathBuf>("dir")
            .map(|x| x.map(ToOwned::to_owned).collect::<Vec<_>>());

        let debug = args.contains_id("debug");

        let max_depth = args
            .get_one::<usize>("max-depth")
            .copied()
            .filter(|depth| *depth > 0);

        let mm_nodes = args.get_one::<String>("nodes").cloned();

        let mm_local_work_dir =
            args.get_one::<String>("local-work-dir").cloned();

        let mm_global_work_dir =
            args.get_one::<String>("global-work-dir").cloned();

        let (count_bytes, count_inodes) = cbi(&args);

        Self {
            dirs,
            debug,
            max_depth,
            mm_nodes,
            mm_local_work_dir,
            mm_global_work_dir,
            count_bytes,
            count_inodes,
        }
    }
}

/// Returns whether to count block usage and/or inode usage.
fn cbi(args: &ArgMatches) -> (bool, bool) {
    let last_block = args
        .indices_of("block")
        .and_then(Iterator::last)
        .unwrap_or_default();

    let last_inodes = args
        .indices_of("inodes")
        .and_then(Iterator::last)
        .unwrap_or_default();

    let last_both = args
        .indices_of("both")
        .and_then(Iterator::last)
        .unwrap_or_default();

    let last = [last_block, last_inodes, last_both]
        .into_iter()
        .max()
        .unwrap();

    if last == last_block {
        (true, false)
    } else if last == last_inodes {
        (false, true)
    } else {
        (true, true)
    }
}

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
            super::cbi(&cli.clone().get_matches_from([
                clap::crate_name!(),
                "--both",
                "--inodes"
            ]))
        );
    }
}
