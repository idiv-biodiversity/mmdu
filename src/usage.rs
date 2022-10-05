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

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use bstr::io::BufReadExt;
use bstr::ByteSlice;
use clap::crate_name;
use tempfile::{tempdir, tempdir_in};

use crate::config::Config;
use crate::log;
use crate::output::output;
use crate::policy;

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

    policy::size(&policy).with_context(|| {
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
    };

    if let Some(ref local_work_dir) = config.mm_local_work_dir {
        command.arg("-s").arg(local_work_dir);
    };

    if let Some(ref global_work_dir) = config.mm_global_work_dir {
        command.arg("-g").arg(global_work_dir);
    };

    log::debug(format!("command: {:?}", command), config);

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

fn sum(dir: &Path, report: &Path, config: &Config) -> io::Result<()> {
    if let Some(depth) = config.max_depth {
        let sizes = sum_depth(dir, depth, report, config)?;

        for (dir, Acc { inodes, bytes }) in sizes {
            // drop files and empty directories
            // they each have only one entry
            if inodes > 1 {
                output(&dir, inodes, bytes, config);
            }
        }
    } else {
        let Acc { inodes, bytes } = sum_total(report).unwrap();
        output(dir, inodes, bytes, config);
    };

    Ok(())
}

fn sum_depth(
    dir: &Path,
    depth: usize,
    report: &Path,
    config: &Config,
) -> io::Result<BTreeMap<PathBuf, Acc>> {
    let report = File::open(report)?;
    let report = BufReader::new(report);

    let mut dir_sums = BTreeMap::new();
    let prefix_depth = Path::new(dir).iter().count();

    for line in report.byte_lines() {
        let line = line?;

        let mut groups = line.splitn_str(2, "--");

        let meta = groups.next().unwrap();

        let bytes = meta.splitn_str(6, " ").nth(4).unwrap();
        let bytes = bytes.to_str().unwrap();
        let bytes: u64 = bytes.parse().unwrap();

        let path = groups.next().unwrap().to_path().unwrap();
        let path_depth = path.iter().count();
        let path_suffix_depth = path_depth - prefix_depth;

        log::debug(format!("path: {:?}", path), config);

        for depth in 0..=depth.min(path_suffix_depth) {
            let prefix: PathBuf =
                path.iter().take(prefix_depth + depth).collect();

            log::debug(format!("prefix: {:?}", prefix), config);

            dir_sums
                .entry(prefix)
                .and_modify(|x| *x += bytes)
                .or_insert_with(|| Acc { inodes: 1, bytes });
        }
    }

    Ok(dir_sums)
}

fn sum_total(report: &Path) -> io::Result<Acc> {
    let mut sum = Acc::default();

    let report = File::open(report)?;
    let report = BufReader::new(report);

    for line in report.byte_lines() {
        let line = line?;

        let bytes = line.splitn_str(6, " ").nth(4).unwrap();
        let bytes = bytes.to_str().unwrap();
        let bytes: u64 = bytes.parse().unwrap();

        sum += bytes;
    }

    Ok(sum)
}

// ----------------------------------------------------------------------------
// accumulator
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct Acc {
    inodes: u64,
    bytes: u64,
}

impl AddAssign<u64> for Acc {
    fn add_assign(&mut self, bytes: u64) {
        *self = Self {
            inodes: self.inodes + 1,
            bytes: self.bytes + bytes,
        };
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use std::path::Path;

    #[test]
    fn total() {
        let acc = super::sum_total(Path::new("src/mmdu.list.size")).unwrap();

        assert_eq!(acc.inodes, 63);
        assert_eq!(acc.bytes, 1_415_269);
    }
}
