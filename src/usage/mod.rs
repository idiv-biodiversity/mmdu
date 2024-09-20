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
mod ncdu;
mod total;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::ops::AddAssign;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use bytesize::{ByteSize, Display};
use clap::crate_name;
use ncdu::FSTree;
use tempfile::{TempDir, tempdir, tempdir_in};

use crate::config::{ByteMode, Config, CountMode, Filter, Report, ReportType};
use mmpolicy::prelude::*;

pub fn run(dir: &Path, config: &Config) -> Result<()> {
    let policy = gen_policy(config);
    let tmp = create_tmp(config)?;
    let policy_path = tmp.path().join(".policy");
    let prefix = tmp.path().join(crate_name!());

    let reports = policy.run(
        dir.as_os_str(),
        policy_path,
        Some(&prefix),
        &config.mm_runoptions,
    )?;

    // from the policies above, we expect exactly one EXTERNAL LIST report
    if reports.len() != 1 {
        return Err(anyhow!(
            "unexpected number of reports: {}",
            reports.len()
        ));
    }

    let mut report = open_report(&reports[0])?;

    let data = collect_data(dir, &mut report, config)?;

    if config.reports.is_empty() {
        return data.write(&mut std::io::stdout(), config);
    }

    for report in &config.reports {
        write_report(&data, report, dir, config)?;
    }

    Ok(())
}

fn gen_policy(config: &Config) -> Policy {
    let mut policy = Policy::new(crate_name!());

    policy.rules.push(Rule::from(RuleType::ExternalList(
        Name("size".into()),
        Exec(String::new()),
    )));

    if config.ncdu() {
        policy.rules.push(Rule::from(RuleType::List(
            Name("size".into()),
            DirectoriesPlus(true),
            vec![Show::Mode, Show::Nlink, Show::FileSize, Show::KbAllocated],
            None,
        )));
    } else {
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
    }

    policy
}

fn create_tmp(config: &Config) -> Result<TempDir> {
    config.mm_runoptions.local_work_dir.as_ref().map_or_else(
        || tempdir().context("creating temdir"),
        |local_work_dir| {
            tempdir_in(local_work_dir).with_context(|| {
                format!("creating tempdir in {}", local_work_dir.display())
            })
        },
    )
}

fn open_report(path: &Path) -> Result<File> {
    File::open(path).with_context(|| {
        format!(
            "opening report {} (this is likely because applying a \
             filter didn't return any results)",
            path.display()
        )
    })
}

fn collect_data(
    dir: &Path,
    report: &mut File,
    config: &Config,
) -> Result<Data> {
    if config.ncdu() {
        ncdu::sum(dir, report).map(Data::Ncdu)
    } else if let Some(depth) = config.max_depth {
        let sizes = depth::sum(dir, depth, report, config.count_links)?;
        Ok(Data::Du(sizes))
    } else {
        let acc = total::sum(report, config.count_links)?;

        let mut sizes = BTreeMap::new();
        sizes.insert(dir.to_owned(), acc);

        Ok(Data::Du(sizes))
    }
}

fn write_report(
    data: &Data,
    report: &Report,
    dir: &Path,
    config: &Config,
) -> Result<()> {
    let mut file = report.create_in(dir)?;

    match (&data, report.tpe) {
        (Data::Du(data), ReportType::Du) => {
            write_du(data, &mut file, config)?;
        }

        (Data::Ncdu(ncdu), ReportType::Du) => {
            if let Some(depth) = config.max_depth {
                let depth_sums = ncdu.to_depth(depth, config);
                write_du(&depth_sums, &mut file, config)?;
            } else {
                let acc = ncdu.to_total(config);
                let mut sizes = BTreeMap::new();
                sizes.insert(dir.to_owned(), acc);
                write_du(&sizes, &mut file, config)?;
            }
        }

        (Data::Ncdu(ncdu), ReportType::Ncdu) => {
            ncdu.write(&mut file)?;
        }

        (Data::Du(_), ReportType::Ncdu) => {
            unreachable!("ncdu report requested but no ncdu data generated")
        }
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// accumulator
// ----------------------------------------------------------------------------

pub enum Data {
    Du(BTreeMap<PathBuf, Acc>),
    Ncdu(FSTree),
}

impl Data {
    fn write(&self, output: &mut impl Write, config: &Config) -> Result<()> {
        match self {
            Self::Du(data) => write_du(data, output, config),
            Self::Ncdu(fstree) => fstree.write(output),
        }
    }
}

fn write_du(
    data: &BTreeMap<PathBuf, Acc>,
    output: &mut impl Write,
    config: &Config,
) -> Result<()> {
    for (dir, Acc { inodes, bytes }) in data {
        let humanized = match config.byte_mode {
            ByteMode::FileSize => humanize(ByteSize::b(*bytes)),
            ByteMode::KBAllocated => humanize(ByteSize::kib(*bytes)),
        };

        let dir = dir.display();

        match config.count_mode {
            CountMode::Both => {
                writeln!(output, "{humanized}\t{inodes}\t{dir}")?;
            }

            CountMode::Bytes => {
                writeln!(output, "{humanized}\t{dir}")?;
            }

            CountMode::Inodes => {
                writeln!(output, "{inodes}\t{dir}")?;
            }
        }
    }

    Ok(())
}

fn humanize(bytes: ByteSize) -> Display {
    bytes.display().iec_short()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Acc {
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
