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

pub fn sum(report: &mut impl Read, count_links: bool) -> Result<Acc> {
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
    use super::*;

    #[test]
    fn parse_hardlinks_once() {
        let source = &mut Entry::EXAMPLE.as_bytes();
        let sum = sum(source, false).unwrap();
        assert_eq!(Acc::from((5, 14336)), sum);
    }

    #[test]
    fn parse_hardlinks_many() {
        let source = &mut Entry::EXAMPLE.as_bytes();
        let sum = sum(source, true).unwrap();
        assert_eq!(Acc::from((8, 17408)), sum);
    }
}
