/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019  Christian Krause                                   *
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

use mktemp::Temp;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::Config;
use crate::log;
use crate::output::*;
use crate::policy;

pub fn run(dir: &str, config: &Config) -> io::Result<()> {
    let tmp = Temp::new_dir()?;

    let mut policy = tmp.to_path_buf();
    policy.push(".policy");

    let mut prefix = tmp.to_path_buf();
    prefix.push(crate_name!());

    policy::size(&policy)?;

    let mut child = Command::new("mmapplypolicy");
    child.arg(dir);

    if let Some(ref nodes) = config.nodes {
        child.args(&["-N", &nodes]);
    };

    let mut child = child
        .args(&["-P", policy.to_str().unwrap()])
        .args(&["-f", prefix.to_str().unwrap()])
        .args(&["-I", "defer"])
        .args(&["-L", "0"])
        .stdout(Stdio::null())
        .spawn()
        .expect("mmapplypolicy command failed to start");

    let ecode = child.wait().expect("failed to wait on child");

    if ecode.success() {
        let mut report = tmp.to_path_buf();
        report.push("mmdu.list.size");

        sum(dir, report, config)?;

        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "mmapplypolicy unsuccessful",
        ))
    }
}

fn sum(dir: &str, report: PathBuf, config: &Config) -> io::Result<()> {
    sum_consume(dir, report, config)
}

fn sum_consume(dir: &str, report: PathBuf, config: &Config) -> io::Result<()> {
    match config.max_depth {
        Some(depth) => {
            let sizes = sum_depth(dir, depth, &report, config)?;

            for (dir, Acc { n, size }) in sizes.iter() {
                // drop files and empty directories
                // they each have only one entry
                if *n > 1 {
                    output(dir.to_str().unwrap(), *size);
                }
            }
        }

        None => {
            let size = sum_total(&report).unwrap();
            output(dir, size);
        }
    };

    Ok(())
}

struct Acc {
    n: u64,
    size: u64,
}

impl Acc {
    fn new(n: u64, size: u64) -> Acc {
        Acc { n, size }
    }
}

impl AddAssign<(u64, u64)> for Acc {
    fn add_assign(&mut self, other: (u64, u64)) {
        let (n, size) = other;

        *self = Acc {
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

    let mut dir_sums = BTreeMap::new();
    let prefix_depth = Path::new(dir).iter().count();

    for line in BufReader::new(report).lines() {
        if line.is_err() {
            log::error(&format!(
                "{}: dropping line from policy report, result will be \
                 incorrect ({:?})",
                dir, line
            ));

            continue;
        };

        let line = line?;

        for cap in policy::RE_SIZE.captures_iter(&line) {
            let size: u64 = cap[1].parse().unwrap();
            let path = Path::new(&cap[2]);
            let path_depth = path.iter().count();
            let path_suffix_depth = path_depth - prefix_depth;

            log::debug(&format!("path: {:?}", path), config);

            for depth in 0..=depth.min(path_suffix_depth) {
                let prefix: PathBuf =
                    path.iter().take(prefix_depth + depth).collect();

                log::debug(&format!("prefix: {:?}", prefix), config);

                dir_sums
                    .entry(prefix)
                    .and_modify(|x| *x += (1u64, size))
                    .or_insert_with(|| Acc::new(1, size));
            }
        }
    }

    Ok(dir_sums)
}

fn sum_total(report: &Path) -> io::Result<u64> {
    let mut sum = 0;

    if report.exists() {
        let report = File::open(report)?;

        for line in BufReader::new(report).lines() {
            let line = line?;

            for cap in policy::RE_SIZE.captures_iter(&line) {
                let size: u64 = cap[1].parse().unwrap();
                sum += size;
            }
        }
    }

    Ok(sum)
}
