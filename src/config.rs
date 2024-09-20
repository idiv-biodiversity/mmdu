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

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use clap::ArgMatches;
use libc::{gid_t, uid_t};

use mmpolicy::prelude::RunOptions;

#[derive(Debug)]
pub struct Config {
    pub filter: Filter,
    pub count_links: bool,
    pub max_depth: Option<usize>,
    pub mm_runoptions: RunOptions,
    pub byte_mode: ByteMode,
    pub count_mode: CountMode,
    pub reports: Vec<Report>,
}

impl TryFrom<&ArgMatches> for Config {
    type Error = anyhow::Error;

    fn try_from(args: &ArgMatches) -> Result<Self> {
        let filter = Filter::try_from(args)?;

        let count_links = args.get_flag("count-links");

        let max_depth = args
            .get_one::<usize>("max-depth")
            .copied()
            .filter(|depth| *depth > 0);

        let mut mm_runoptions = RunOptions::from(args);
        mm_runoptions.action = Some("defer".into());
        mm_runoptions.choice_algorithm = Some("fast".into());
        mm_runoptions.information_level = Some("0".into());

        let byte_mode = if args.get_flag("kb-allocated") {
            ByteMode::KBAllocated
        } else {
            ByteMode::FileSize
        };

        let count_mode = CountMode::from(args);

        let mut reports = vec![];

        if let Some(path) = args.get_one::<String>("report-du") {
            reports.push(Report {
                path_or_pattern: path.to_owned(),
                tpe: ReportType::Du,
            });
        }

        if let Some(path) = args.get_one::<String>("report-ncdu") {
            reports.push(Report {
                path_or_pattern: path.to_owned(),
                tpe: ReportType::Ncdu,
            });
        }

        Ok(Self {
            filter,
            count_links,
            max_depth,
            mm_runoptions,
            byte_mode,
            count_mode,
            reports,
        })
    }
}

impl Config {
    pub fn ncdu(&self) -> bool {
        self.reports
            .iter()
            .any(|o| matches!(o.tpe, ReportType::Ncdu))
    }
}

#[derive(Debug)]
pub enum Filter {
    Group(gid_t),
    User(uid_t),
    None,
}

impl Filter {
    fn group_to_gid(group: &str) -> Result<gid_t> {
        let is_numeric = group.chars().all(char::is_numeric);

        if is_numeric {
            group
                .parse::<gid_t>()
                .with_context(|| format!("failed to parse {group} as `gid_t`"))
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
        let is_numeric = user.chars().all(char::is_numeric);

        if is_numeric {
            user.parse::<uid_t>()
                .with_context(|| format!("failed to parse {user} as `uid_t`"))
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

#[derive(Debug)]
pub struct Report {
    pub path_or_pattern: String,
    pub tpe: ReportType,
}

impl Report {
    pub fn create_in(&self, base: &Path) -> Result<File> {
        let path = &self.path_or_pattern;

        let path = if path.starts_with("{}/") {
            base.join(path.replace("{}/", ""))
        } else {
            PathBuf::from(path)
        };

        File::create(&path).with_context(|| {
            format!("creating report file {}", path.display())
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ReportType {
    Du,
    Ncdu,
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
