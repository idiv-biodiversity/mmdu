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

use libc::uid_t;

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

fn policy_group(group: uid_t, attribute: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR({attribute}))
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
  SHOW(VARCHAR({attribute}))
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
  SHOW(VARCHAR({attribute}))
"
    )
}
