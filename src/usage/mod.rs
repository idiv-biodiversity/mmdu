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
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};
use clap::crate_name;
use tempfile::{tempdir, tempdir_in};

use crate::config::Config;
use crate::output::output;

pub fn run(dir: &Path, config: &Config) -> Result<()> {
    let tmp = if let Some(ref local_work_dir) = config.mm_local_work_dir {
        tempdir_in(local_work_dir).with_context(|| {
            format!("creating tempdir in {}", local_work_dir.display())
        })?
    } else {
        tempdir().context("creating temdir")?
    };

    let policy = tmp.path().join(".policy");
    let prefix = tmp.path().join(crate_name!());

    crate::policy::size(&policy, config).with_context(|| {
        format!("writing policy file to {}", policy.display())
    })?;

    let mut command = Command::new("mmapplypolicy");
    command
        .arg(dir)
        .args(["-P", policy.to_str().unwrap()])
        .args(["-f", prefix.to_str().unwrap()])
        .args(["--choice-algorithm", "fast"])
        .args(["-I", "defer"])
        .args(["-L", "0"]);

    if let Some(ref nodes) = config.mm_nodes {
        command.args(["-N", nodes]);
    }

    if let Some(ref local_work_dir) = config.mm_local_work_dir {
        command.arg("-s").arg(local_work_dir);
    }

    if let Some(ref global_work_dir) = config.mm_global_work_dir {
        command.arg("-g").arg(global_work_dir);
    }

    #[cfg(feature = "log")]
    log::debug!("command: {command:?}");

    let mut child = command
        .stdout(Stdio::null())
        .spawn()
        .context("`mmapplypolicy` command failed to start")?;

    let ecode = child.wait().context("failed waiting on `mmapplypolicy`")?;

    if ecode.success() {
        let report = tmp.path().join("mmdu.list.size");

        sum(dir, &report, config)?;

        Ok(())
    } else {
        // ALLOW if let is easier to comprehend
        #[allow(clippy::option_if_let_else)]
        let e = if let Some(rc) = ecode.code() {
            anyhow!("`mmapplypolicy` failed with exit status {rc}")
        } else {
            anyhow!("`mmapplypolicy` failed")
        };

        Err(e)
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
