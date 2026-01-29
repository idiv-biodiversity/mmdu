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

use std::collections::HashMap;
use std::io::{BufReader, Read};

use anyhow::{Context, Result};
use bstr::io::BufReadExt;

use crate::policy::Entry;
use crate::usage::Acc;

pub fn sum(report: impl Read, count_links: bool) -> Result<Acc> {
    let mut sum = Acc::default();
    let mut hard_links: HashMap<String, u64> = HashMap::new();

    for line in BufReader::new(report).byte_lines() {
        let line = line.context("reading line from policy report")?;

        let entry = Entry::try_from(&line)
            .context("parsing line from policy report")?;

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
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use indoc::indoc;

    use super::*;

    const SOURCE: &str = indoc! {"
        1 1 0  4096 1 -- /data/test
        1 1 0  1024 3 -- /data/test/foo
        1 1 0  1024 3 -- /data/test/bar
        1 1 0  1024 3 -- /data/test/baz
        2 1 0  1024 2 -- /data/test/other
    "};

    const INVALID: &str = indoc! {"
        1 1 0  4096 1 -- /data/test
        1 1 0  1024 2 -- /data/test/foo
        1 1 0  3 -- /data/test/bar
    "};

    #[test]
    fn parse_hardlinks_once() {
        let sum = sum(SOURCE.as_bytes(), false).unwrap();
        assert_eq!(Acc::from((3, 6144)), sum);
    }

    #[test]
    fn parse_hardlinks_many() {
        let sum = sum(SOURCE.as_bytes(), true).unwrap();
        assert_eq!(Acc::from((5, 8192)), sum);
    }

    #[test]
    fn parse_invalid() {
        let result = sum(INVALID.as_bytes(), false);
        assert!(result.is_err());

        let error = format!("{:#?}", result.unwrap_err());
        assert!(error.contains("/data/test/bar"));
    }
}
