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
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bstr::io::BufReadExt;

use crate::policy::Entry;
use crate::usage::Acc;

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

pub fn sum(
    dir: &Path,
    depth: usize,
    report: &mut impl Read,
    count_links: bool,
) -> Result<BTreeMap<PathBuf, Acc>> {
    let report = BufReader::new(report);

    let mut sums: HashMap<PathBuf, DepthAcc> = HashMap::new();

    let prefix_depth = dir.iter().count();

    for line in report.byte_lines() {
        let line = line.context("reading line from policy report")?;
        let entry = Entry::try_from(&line)
            .context("parsing line from policy report")?;

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

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use indoc::indoc;

    use super::*;

    const SOURCE: &str = indoc! {"
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

    #[test]
    fn parse_hardlinks_once() {
        let mut expected = BTreeMap::new();
        expected.insert("/data/test".into(), Acc::from((5, 14336)));
        expected.insert("/data/test/a".into(), Acc::from((2, 5120)));
        expected.insert("/data/test/b".into(), Acc::from((3, 6144)));

        let sum =
            sum(Path::new("/data/test"), 1, &mut SOURCE.as_bytes(), false)
                .unwrap();

        assert_eq!(expected, sum);
    }

    #[test]
    fn parse_hardlinks_many() {
        let mut expected = BTreeMap::new();
        expected.insert("/data/test".into(), Acc::from((10, 19456)));
        expected.insert("/data/test/a".into(), Acc::from((3, 6144)));
        expected.insert("/data/test/b".into(), Acc::from((3, 6144)));

        let sum =
            sum(Path::new("/data/test"), 1, &mut SOURCE.as_bytes(), true)
                .unwrap();

        assert_eq!(expected, sum);
    }
}
