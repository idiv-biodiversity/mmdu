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

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
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

    policy::size(&policy, config).with_context(|| {
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

    log::debug(format!("command: {command:?}"), config.debug);

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
        let sizes = sum_depth(dir, depth, report, config.debug)?;

        for (dir, Acc { inodes, bytes }) in sizes {
            // drop files and empty directories
            // they each have only one entry
            if inodes > 1 {
                output(&dir, inodes, bytes, config);
            }
        }
    } else {
        let Acc { inodes, bytes } = sum_total(report)?;
        output(dir, inodes, bytes, config);
    };

    Ok(())
}

fn sum_depth(
    dir: &Path,
    depth: usize,
    report: impl Read,
    debug: bool,
) -> Result<BTreeMap<PathBuf, Acc>> {
    let report = BufReader::new(report);

    let mut dir_sums = BTreeMap::new();
    let prefix_depth = Path::new(dir).iter().count();

    for line in report.byte_lines() {
        let line = line?;

        let mut groups = line.splitn_str(2, " -- ");

        let meta = groups.next().unwrap();

        let bytes = meta.splitn_str(6, " ").nth(4).unwrap();
        let bytes = bytes.to_str().unwrap();
        let bytes: u64 = bytes.parse().unwrap();

        let path = groups.next().unwrap().to_path().unwrap();
        let path_depth = path.iter().count();
        let path_suffix_depth = path_depth - prefix_depth;

        log::debug(format!("path: {path:?}"), debug);

        for depth in 0..=depth.min(path_suffix_depth) {
            let prefix: PathBuf =
                path.iter().take(prefix_depth + depth).collect();

            log::debug(format!("prefix: {prefix:?}"), debug);

            dir_sums
                .entry(prefix)
                .and_modify(|x| *x += bytes)
                .or_insert_with(|| Acc { inodes: 1, bytes });
        }
    }

    Ok(dir_sums)
}

fn sum_total(report: impl Read) -> Result<Acc> {
    let report = BufReader::new(report);

    let mut sum = Acc::default();

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[cfg(test)]
impl From<(u64, u64)> for Acc {
    fn from((inodes, bytes): (u64, u64)) -> Self {
        Self { inodes, bytes }
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn total_simple() {
        let source = "1 1 0  1024 1 -- /data/test/foo\n\
                      2 1 0  1024 1 -- /data/test/bar\n";

        let result = sum_total(source.as_bytes()).unwrap();

        assert_eq!(Acc::from((2, 2048)), result);
    }

    #[test]
    fn depth_simple() {
        let source = "1 1 0  1024 1 -- /data/test/a/foo\n\
                      2 1 0  1024 1 -- /data/test/b/bar\n";

        let mut expected = BTreeMap::new();
        expected.insert("/data/test".into(), Acc::from((2, 2048)));
        expected.insert("/data/test/a".into(), Acc::from((1, 1024)));
        expected.insert("/data/test/b".into(), Acc::from((1, 1024)));

        let result =
            sum_depth(Path::new("/data/test"), 1, source.as_bytes(), false)
                .unwrap();

        assert_eq!(expected, result);
    }
}
