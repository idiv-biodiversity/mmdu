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

use anyhow::{anyhow, Context, Result};
use clap::ArgMatches;
use libc::uid_t;

pub fn get() -> Result<Config> {
    let cli = crate::cli::build();
    let args = cli.get_matches();
    Config::try_from(args)
}

#[derive(Debug)]
pub struct Config {
    pub dirs: Option<Vec<PathBuf>>,
    pub debug: bool,
    pub filter: Filter,
    pub max_depth: Option<usize>,
    pub mm_nodes: Option<String>,
    pub mm_local_work_dir: Option<PathBuf>,
    pub mm_global_work_dir: Option<PathBuf>,
    pub byte_mode: ByteMode,
    pub count_mode: CountMode,
}

impl TryFrom<ArgMatches> for Config {
    type Error = anyhow::Error;

    fn try_from(args: ArgMatches) -> Result<Self> {
        let dirs = args
            .get_many::<PathBuf>("dir")
            .map(|x| x.map(ToOwned::to_owned).collect::<Vec<_>>());

        let debug = args.get_one::<bool>("debug").copied().unwrap_or_default();

        let filter = Filter::try_from(&args)?;

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

        Ok(Self {
            dirs,
            debug,
            filter,
            max_depth,
            mm_nodes,
            mm_local_work_dir,
            mm_global_work_dir,
            byte_mode,
            count_mode,
        })
    }
}

#[derive(Debug)]
pub enum Filter {
    Group(uid_t),
    User(uid_t),
    None,
}

impl Filter {
    fn group_to_gid(group: &str) -> Result<uid_t> {
        let is_gid = group.chars().all(char::is_numeric);

        if is_gid {
            let gid = group.parse::<uid_t>().with_context(|| {
                format!("failed to parse {group} as `uid_t`")
            })?;

            let entry = pwd_grp::getgrgid(gid).with_context(|| {
                format!("searching group database for {group}")
            })?;

            entry
                .ok_or_else(|| anyhow!("group {group} not found"))
                .map(|_| gid)
        } else {
            let entry = pwd_grp::getgrnam(group).with_context(|| {
                format!("searching group database for {group}")
            })?;

            entry
                .ok_or_else(|| anyhow!("group {group} not found"))
                .map(|group| group.gid)
        }
    }

    fn user_to_uid(user: &str) -> Result<uid_t> {
        let is_uid = user.chars().all(char::is_numeric);

        if is_uid {
            let uid = user.parse::<uid_t>().with_context(|| {
                format!("failed to parse {user} as `uid_t`")
            })?;

            let entry = pwd_grp::getpwuid(uid).with_context(|| {
                format!("searching passwd database for {user}")
            })?;

            entry
                .ok_or_else(|| anyhow!("user {user} not found"))
                .map(|_| uid)
        } else {
            let entry = pwd_grp::getpwnam(user).with_context(|| {
                format!("searching passwd database for {user}")
            })?;

            entry
                .ok_or_else(|| anyhow!("user {user} not found"))
                .map(|passwd| passwd.uid)
        }
    }
}

impl TryFrom<&ArgMatches> for Filter {
    type Error = anyhow::Error;

    fn try_from(args: &ArgMatches) -> Result<Self> {
        let group = args.get_one::<String>("group");
        let user = args.get_one::<String>("user");

        match (group, user) {
            (None, None) => Ok(Self::None),
            (Some(group), None) => Self::group_to_gid(group).map(Self::Group),
            (None, Some(user)) => Self::user_to_uid(user).map(Self::User),
            (Some(_), Some(_)) => {
                unreachable!("{}", crate::cli::CONFLICT_FILTER)
            }
        }
    }
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
