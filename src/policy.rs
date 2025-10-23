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

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use bstr::ByteSlice;
use libc::{gid_t, uid_t};

use crate::config::{Config, Filter};

pub fn size(file: &Path, config: &Config) -> io::Result<()> {
    let mut file = File::create(file)?;

    let attribute = config.byte_mode.policy_attribute();

    let content = match &config.filter {
        Filter::Group(group) => policy_group(*group, attribute),
        Filter::User(user) => policy_user(*user, attribute),
        Filter::None => policy_default(attribute),
    };

    file.write_all(content.as_bytes())?;

    Ok(())
}

fn policy_group(group: gid_t, attribute: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR({attribute}) || ' ' || VARCHAR(NLINK))
  WHERE GROUP_ID = {group}
"
    )
}

fn policy_user(user: uid_t, attribute: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR({attribute}) || ' ' || VARCHAR(NLINK))
  WHERE USER_ID = {user}
"
    )
}

fn policy_default(attribute: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR({attribute}) || ' ' || VARCHAR(NLINK))
"
    )
}

// inode generation snapid  X Y Z -- path
pub struct Entry<'a>(Vec<&'a [u8]>, &'a [u8]);

impl Entry<'_> {
    const INVALID: &'static str = "invalid line in policy report";

    pub fn inode_str(&self) -> Result<&str> {
        self.0[0]
            .to_str()
            .context("reading inode field from policy report")
    }

    pub fn bytes_str(&self) -> Result<&str> {
        self.0[4]
            .to_str()
            .context("reading bytes field from policy report")
    }

    pub fn bytes(&self) -> Result<u64> {
        self.bytes_str().and_then(|s| {
            s.parse::<u64>()
                .context("parsing bytes field from policy report")
        })
    }

    pub fn nlink_str(&self) -> Result<&str> {
        self.0[5]
            .to_str()
            .context("reading number of links field from policy report")
    }

    pub fn path(&self) -> Result<&Path> {
        self.1
            .to_path()
            .context("parsing path field from policy report")
    }
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
