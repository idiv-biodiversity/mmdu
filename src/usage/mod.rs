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

mod depth;
mod total;

use std::fs::File;
use std::ops::AddAssign;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use clap::crate_name;
use tempfile::{tempdir, tempdir_in};

use crate::config::{ByteMode, Config, Filter};
use crate::output::output;
use mmpolicy::prelude::*;

pub fn run(dir: &Path, config: &Config) -> Result<()> {
    let mut policy = Policy::new(crate_name!());

    policy.rules.push(Rule::from(RuleType::ExternalList(
        Name("size".into()),
        Exec(String::new()),
    )));

    let byte_mode = match config.byte_mode {
        ByteMode::FileSize => Show::FileSize,
        ByteMode::KBAllocated => Show::KbAllocated,
    };

    let filter = match &config.filter {
        Filter::Group(group) => Some(Where::Group(*group)),
        Filter::User(user) => Some(Where::User(*user)),
        Filter::None => None,
    };

    policy.rules.push(Rule::from(RuleType::List(
        Name("size".into()),
        DirectoriesPlus(true),
        vec![byte_mode, Show::Nlink],
        filter,
    )));

    let tmp =
        if let Some(local_work_dir) = &config.mm_runoptions.local_work_dir {
            tempdir_in(local_work_dir).with_context(|| {
                format!("creating tempdir in {}", local_work_dir.display())
            })?
        } else {
            tempdir().context("creating temdir")?
        };

    let policy_path = tmp.path().join(".policy");
    let prefix = tmp.path().join(crate_name!());

    let reports = policy.run(
        dir.as_os_str(),
        policy_path,
        Some(&prefix),
        &config.mm_runoptions,
    )?;

    // from the policies above, we expect exactly one external list
    if reports.len() == 1 {
        sum(dir, &reports[0], config)
    } else {
        Err(anyhow!("unexpected number of reports: {}", reports.len()))
    }
}

fn sum(dir: &Path, report: &Path, config: &Config) -> Result<()> {
    let report = File::open(report).with_context(|| {
        format!(
            "opening report {} (this is likely because applying a \
             filter didn't return any results)",
            report.display()
        )
    })?;

    if let Some(depth) = config.max_depth {
        let sizes = depth::sum(dir, depth, report, config.count_links)?;

        for (dir, Acc { inodes, bytes }) in sizes {
            output(&dir, inodes, bytes, config);
        }
    } else {
        let Acc { inodes, bytes } = total::sum(report, config.count_links)?;
        output(dir, inodes, bytes, config);
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// accumulator
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Acc {
    inodes: u64,
    bytes: u64,
}

impl Acc {
    const fn new(bytes: u64) -> Self {
        Self { inodes: 1, bytes }
    }
}

impl AddAssign<u64> for Acc {
    fn add_assign(&mut self, bytes: u64) {
        *self = Self {
            inodes: self.inodes + 1,
            bytes: self.bytes + bytes,
        };
    }
}

#[cfg(test)]
impl From<(u64, u64)> for Acc {
    fn from((inodes, bytes): (u64, u64)) -> Self {
        Self { inodes, bytes }
    }
}
