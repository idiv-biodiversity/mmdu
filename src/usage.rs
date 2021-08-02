/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019-2021  Christian Krause                              *
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

use bstr::io::BufReadExt;
use bstr::ByteSlice;
use clap::crate_name;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::{tempdir, tempdir_in};

use crate::config::Config;
use crate::log;
use crate::output::output;
use crate::policy;

pub fn run(dir: &str, config: &Config) -> io::Result<()> {
    let tmp = if let Some(ref local_work_dir) = config.local_work_dir {
        tempdir_in(local_work_dir)?
    } else {
        tempdir()?
    };

    let policy = tmp.path().join(".policy");
    let prefix = tmp.path().join(crate_name!());

    policy::size(&policy)?;

    let mut command = Command::new("mmapplypolicy");
    command
        .arg(dir)
        .args(&["-P", policy.to_str().unwrap()])
        .args(&["-f", prefix.to_str().unwrap()])
        .args(&["--choice-algorithm", "fast"])
        .args(&["-I", "defer"])
        .args(&["-L", "0"]);

    if let Some(ref nodes) = config.nodes {
        command.args(&["-N", nodes]);
    };

    if let Some(ref local_work_dir) = config.local_work_dir {
        command.args(&["-s", local_work_dir]);
    };

    if let Some(ref global_work_dir) = config.global_work_dir {
        command.args(&["-g", global_work_dir]);
    };

    log::debug(format!("command: {:?}", command), config);

    let mut child = command
        .stdout(Stdio::null())
        .spawn()
        .expect("mmapplypolicy command failed to start");

    let ecode = child.wait().expect("failed waiting on mmapplypolicy");

    if ecode.success() {
        let report = tmp.path().join("mmdu.list.size");

        sum(dir, &report, config)?;

        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, "mmapplypolicy failed"))
    }
}

fn sum(dir: &str, report: &Path, config: &Config) -> io::Result<()> {
    sum_consume(dir, report, config)
}

fn sum_consume(dir: &str, report: &Path, config: &Config) -> io::Result<()> {
    if let Some(depth) = config.max_depth {
        let sizes = sum_depth(dir, depth, report, config)?;

        for (dir, Acc { n, size }) in &sizes {
            // drop files and empty directories
            // they each have only one entry
            if *n > 1 {
                output(dir.to_string_lossy(), *size);
            }
        }
    } else {
        let size = sum_total(report).unwrap();
        output(dir, size);
    };

    Ok(())
}

struct Acc {
    n: u64,
    size: u64,
}

impl Acc {
    const fn new(n: u64, size: u64) -> Self {
        Self { n, size }
    }
}

impl AddAssign<(u64, u64)> for Acc {
    fn add_assign(&mut self, other: (u64, u64)) {
        let (n, size) = other;

        *self = Self {
            n: self.n + n,
            size: self.size + size,
        };
    }
}

fn sum_depth(
    dir: &str,
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

        let size = meta.splitn_str(6, " ").nth(4).unwrap();
        let size = size.to_str().unwrap();
        let size: u64 = size.parse().unwrap();

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
                .and_modify(|x| *x += (1, size))
                .or_insert_with(|| Acc::new(1, size));
        }
    }

    Ok(dir_sums)
}

fn sum_total(report: &Path) -> io::Result<u64> {
    let mut sum = 0;

    if report.exists() {
        let report = File::open(report)?;
        let report = BufReader::new(report);

        for line in report.byte_lines() {
            let line = line?;

            let size = line.splitn_str(6, " ").nth(4).unwrap();
            let size = size.to_str().unwrap();
            let size: u64 = size.parse().unwrap();

            sum += size;
        }
    }

    Ok(sum)
}
