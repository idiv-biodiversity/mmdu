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
    pub byte_mode: ByteMode,
    pub count_mode: CountMode,
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

        let byte_mode = if args.get_flag("kb-allocated") {
            ByteMode::KBAllocated
        } else {
            ByteMode::FileSize
        };

        let count_mode = CountMode::from(&args);

        Self {
            dirs,
            debug,
            filter,
            max_depth,
            mm_nodes,
            mm_local_work_dir,
            mm_global_work_dir,
            byte_mode,
            count_mode,
        }
    }
}

#[derive(Debug)]
pub enum Filter {
    Group(String),
    User(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteMode {
    FileSize,
    KBAllocated,
}

impl ByteMode {
    pub const fn policy_attribute(self) -> &'static str {
        match self {
            Self::FileSize => "FILE_SIZE",
            Self::KBAllocated => "KB_ALLOCATED",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountMode {
    Inodes,
    Bytes,
    Both,
}

impl From<&ArgMatches> for CountMode {
    fn from(args: &ArgMatches) -> Self {
        let block = args.get_flag("block");
        let inodes = args.get_flag("inodes");
        let both = args.get_flag("both");

        // most conditions aren't possible due to `overrides_with_all`
        match (block, inodes, both) {
            (_, true, _) => Self::Inodes,
            (_, _, true) => Self::Both,
            _ => Self::Bytes,
        }
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
impl From<ArgMatches> for CountMode {
    fn from(args: ArgMatches) -> Self {
        Self::from(&args)
    }
}

#[cfg(test)]
mod test {
    use super::CountMode;

    #[test]
    fn count_mode() {
        let cli = crate::cli::build();

        assert_eq!(
            CountMode::Bytes,
            CountMode::from(
                cli.clone().get_matches_from([clap::crate_name!()])
            )
        );

        assert_eq!(
            CountMode::Bytes,
            CountMode::from(
                cli.clone()
                    .get_matches_from([clap::crate_name!(), "--block"])
            )
        );

        assert_eq!(
            CountMode::Inodes,
            CountMode::from(
                cli.clone()
                    .get_matches_from([clap::crate_name!(), "--inodes"])
            )
        );

        assert_eq!(
            CountMode::Both,
            CountMode::from(
                cli.clone()
                    .get_matches_from([clap::crate_name!(), "--both"])
            )
        );

        assert_eq!(
            CountMode::Inodes,
            CountMode::from(cli.clone().get_matches_from([
                clap::crate_name!(),
                "--block",
                "--inodes"
            ]))
        );

        assert_eq!(
            CountMode::Both,
            CountMode::from(cli.clone().get_matches_from([
                clap::crate_name!(),
                "--block",
                "--both"
            ]))
        );

        assert_eq!(
            CountMode::Bytes,
            CountMode::from(cli.clone().get_matches_from([
                clap::crate_name!(),
                "--inodes",
                "--block"
            ]))
        );

        assert_eq!(
            CountMode::Both,
            CountMode::from(cli.clone().get_matches_from([
                clap::crate_name!(),
                "--inodes",
                "--both"
            ]))
        );

        assert_eq!(
            CountMode::Bytes,
            CountMode::from(cli.clone().get_matches_from([
                clap::crate_name!(),
                "--both",
                "--block"
            ]))
        );

        assert_eq!(
            CountMode::Inodes,
            CountMode::from(cli.get_matches_from([
                clap::crate_name!(),
                "--both",
                "--inodes"
            ]))
        );
    }
}
