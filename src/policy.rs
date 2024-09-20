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

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use bstr::ByteSlice;

// inode generation snapid  X Y Z -- path
pub struct Entry<'a>(Vec<&'a [u8]>, &'a [u8]);

impl Entry<'_> {
    const INVALID: &'static str = "invalid line in policy report";

    pub fn inode_str(&self) -> Result<&str> {
        self.0[0].to_str().context("reading inode field")
    }

    pub fn bytes_str(&self) -> Result<&str> {
        self.0[4].to_str().context("reading bytes field")
    }

    pub fn bytes(&self) -> Result<u64> {
        self.bytes_str()
            .and_then(|s| s.parse().context("parsing bytes field"))
    }

    pub fn nlink_str(&self) -> Result<&str> {
        self.0[5].to_str().context("reading number of links field")
    }

    pub fn path(&self) -> Result<&Path> {
        self.1.to_path().context("parsing path field")
    }

    #[cfg(test)]
    pub const EXAMPLE: &'static str = indoc::indoc! {"
        1 0 0  4096 1 -- /data/test
        2 0 0  4096 1 -- /data/test/a
        3 0 0  1024 1 -- /data/test/a/baz
        4 0 0  1024 4 -- /data/test/bar
        4 0 0  1024 4 -- /data/test/foo
        4 0 0  1024 4 -- /data/test/a/foo
        4 0 0  1024 4 -- /data/test/b/bar
        5 0 0  4096 1 -- /data/test/b
    "};
}

impl<'a> TryFrom<&'a Vec<u8>> for Entry<'a> {
    type Error = anyhow::Error;

    fn try_from(line: &'a Vec<u8>) -> Result<Self> {
        let groups = line.split_str(" -- ").collect::<Vec<_>>();

        if groups.len() != 2 {
            return Err(anyhow!(Entry::INVALID));
        }

        let fields = groups[0].splitn_str(7, " ").take(6).collect::<Vec<_>>();
        let path = groups[1];

        if fields.len() == 6 {
            Ok(Self(fields, path))
        } else {
            Err(anyhow!(Entry::INVALID))
        }
    }
}

// inode generation snapid  X Y Z -- path
pub struct NcduEntry<'a>(Vec<&'a [u8]>, &'a [u8]);

impl NcduEntry<'_> {
    pub fn inode_str(&self) -> Result<&str> {
        self.0[0].to_str().context("reading inode field")
    }

    pub fn inode(&self) -> Result<u64> {
        self.inode_str().and_then(|s| {
            s.parse()
                .with_context(|| format!("parsing inode field: \"{s}\""))
        })
    }

    pub fn mode_str(&self) -> Result<&str> {
        self.0[3].to_str().context("reading MODE field")
    }

    pub fn nlink_str(&self) -> Result<&str> {
        self.0[4].to_str().context("reading NLINK field")
    }

    pub fn nlink(&self) -> Result<u32> {
        self.nlink_str().and_then(|s| {
            s.parse()
                .with_context(|| format!("parsing NLINK field: \"{s}\""))
        })
    }

    pub fn file_size_str(&self) -> Result<&str> {
        self.0[5].to_str().context("reading FILE_SIZE field")
    }

    pub fn file_size(&self) -> Result<u64> {
        self.file_size_str()
            .and_then(|s| s.parse().context("parsing FILE_SIZE field"))
    }

    pub fn kb_allocated_str(&self) -> Result<&str> {
        self.0[6].to_str().context("reading KB_ALLOCATED field")
    }

    pub fn kb_allocated(&self) -> Result<u64> {
        self.kb_allocated_str()
            .and_then(|s| s.parse().context("parsing KB_ALLOCATED field"))
    }

    pub fn path(&self) -> Result<&Path> {
        self.1.to_path().context("parsing path field")
    }

    #[cfg(test)]
    pub const EXAMPLE: &'static str = indoc::indoc! {"
        1 0 0  drwx------ 1 4096 0 -- /data/test
        2 0 0  drwxr-xr-x 1 4096 0 -- /data/test/a
        3 0 0  -rw-r--r-- 1 1024 0 -- /data/test/a/baz
        4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/bar
        4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/foo
        4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/a/foo
        4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/b/bar
        5 0 0  drwxr-xr-x 1 4096 0 -- /data/test/b
    "};
}

impl<'a> TryFrom<&'a Vec<u8>> for NcduEntry<'a> {
    type Error = anyhow::Error;

    fn try_from(line: &'a Vec<u8>) -> Result<Self> {
        let groups = line.split_str(" -- ").collect::<Vec<_>>();

        if groups.len() != 2 {
            return Err(anyhow!(
                "no \" -- \" separator (splits in {}): {}",
                groups.len(),
                line.to_str_lossy()
            ));
        }

        let fields = groups[0]
            .splitn_str(8, " ")
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        let path = groups[1];

        if fields.len() == 7 {
            Ok(Self(fields, path))
        } else {
            Err(anyhow!(
                "incorrect number of fields ({}): {}",
                fields.len(),
                line.to_str_lossy()
            ))
        }
    }
}
