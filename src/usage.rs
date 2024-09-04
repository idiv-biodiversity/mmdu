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

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow};
use bstr::io::BufReadExt;
use clap::crate_name;
use tempfile::{tempdir, tempdir_in};

use crate::config::Config;
use crate::output::output;
use crate::policy::{self, Entry};

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
        let sizes = sum_depth(dir, depth, report, config.count_links)?;

        for (dir, Acc { inodes, bytes }) in sizes {
            output(&dir, inodes, bytes, config);
        }
    } else {
        let Acc { inodes, bytes } = sum_total(report, config.count_links)?;
        output(dir, inodes, bytes, config);
    }

    Ok(())
}

fn sum_depth(
    dir: &Path,
    depth: usize,
    report: impl Read,
    count_links: bool,
) -> Result<BTreeMap<PathBuf, Acc>> {
    let report = BufReader::new(report);

    let mut sums: HashMap<PathBuf, DepthAcc> = HashMap::new();

    let prefix_depth = Path::new(dir).iter().count();

    for line in report.byte_lines() {
        let line = line.context("reading line from policy report")?;
        let entry = Entry::try_from(&line)?;

        let bytes = entry.bytes()?;
        let nlink = entry.nlink_str()?;
        let inode = entry.inode_str()?;

        let path = entry.path()?;
        let path_depth = path.iter().count();
        let path_suffix_depth = path_depth - prefix_depth;

        for depth in 0..=depth.min(path_suffix_depth) {
            let prefix: PathBuf =
                path.iter().take(prefix_depth + depth).collect();

            if count_links || nlink == "1" {
                sums.entry(prefix)
                    .and_modify(|v| v.acc += bytes)
                    .or_insert_with(|| DepthAcc::new(bytes));
            } else {
                sums.entry(prefix)
                    .and_modify(|v| {
                        let inode = v
                            .hard_links
                            .entry(inode.to_owned())
                            .and_modify(|c| *c += 1)
                            .or_insert(1);

                        if *inode == 1 {
                            v.acc += bytes;
                        }
                    })
                    .or_insert_with(|| {
                        let mut hard_links = HashMap::new();
                        hard_links.insert(inode.to_owned(), 1);

                        DepthAcc {
                            acc: Acc::new(bytes),
                            hard_links,
                        }
                    });
            }
        }
    }

    Ok(sums
        .into_iter()
        .filter_map(|(path, v)| (v.acc.inodes > 1).then_some((path, v.acc)))
        .collect())
}

fn sum_total(report: impl Read, count_links: bool) -> Result<Acc> {
    let mut sum = Acc::default();
    let mut hard_links: HashMap<String, u64> = HashMap::new();

    for line in BufReader::new(report).byte_lines() {
        let line = line.context("reading line from policy report")?;
        let entry = Entry::try_from(&line)?;

        let bytes = entry.bytes()?;

        if count_links {
            sum += bytes;
            continue;
        }

        let nlink = entry.nlink_str()?;

        if nlink == "1" {
            sum += bytes;
            continue;
        }

        let inode = entry.inode_str()?;
        let inode = hard_links
            .entry(inode.to_owned())
            .and_modify(|c| *c += 1)
            .or_insert(1);

        if *inode == 1 {
            sum += bytes;
        }
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

struct DepthAcc {
    acc: Acc,
    hard_links: HashMap<String, u64>,
}

impl DepthAcc {
    fn new(bytes: u64) -> Self {
        Self {
            acc: Acc::new(bytes),
            hard_links: HashMap::new(),
        }
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use indoc::indoc;

    use super::*;

    #[test]
    fn total() {
        let source = indoc! {"
            1 1 0  4096 1 -- /data/test
            1 1 0  1024 3 -- /data/test/foo
            1 1 0  1024 3 -- /data/test/bar
            1 1 0  1024 3 -- /data/test/baz
            2 1 0  1024 2 -- /data/test/other
        "};

        let once = sum_total(source.as_bytes(), false).unwrap();
        assert_eq!(Acc::from((3, 6144)), once);

        let many = sum_total(source.as_bytes(), true).unwrap();
        assert_eq!(Acc::from((5, 8192)), many);
    }

    #[test]
    fn depth() {
        let source = indoc! {"
            1 1 0  4096 1 -- /data/test
            1 1 0  1024 5 -- /data/test/foo
            1 1 0  1024 5 -- /data/test/bar
            2 1 0  1024 2 -- /data/test/other
            1 1 0  4096 1 -- /data/test/a
            1 1 0  1024 5 -- /data/test/a/foo
            1 1 0  1024 5 -- /data/test/a/bar
            1 1 0  4096 1 -- /data/test/b
            1 1 0  1024 5 -- /data/test/b/foo
            2 1 0  1024 2 -- /data/test/b/other
        "};

        let mut once = BTreeMap::new();
        once.insert("/data/test".into(), Acc::from((5, 14336)));
        once.insert("/data/test/a".into(), Acc::from((2, 5120)));
        once.insert("/data/test/b".into(), Acc::from((3, 6144)));

        let result =
            sum_depth(Path::new("/data/test"), 1, source.as_bytes(), false)
                .unwrap();

        assert_eq!(once, result);

        let mut many = BTreeMap::new();
        many.insert("/data/test".into(), Acc::from((10, 19456)));
        many.insert("/data/test/a".into(), Acc::from((3, 6144)));
        many.insert("/data/test/b".into(), Acc::from((3, 6144)));

        let result =
            sum_depth(Path::new("/data/test"), 1, source.as_bytes(), true)
                .unwrap();

        assert_eq!(many, result);
    }
}
