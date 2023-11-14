/* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
 *                                                                           *
 *  Copyright  (C)  2019-2023  Christian Krause                              *
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

use crate::config::{Config, Filter};

pub fn size(file: &Path, config: &Config) -> io::Result<()> {
    let mut file = File::create(file)?;

    let content = match &config.filter {
        Some(Filter::Group(group)) => policy_group(group),
        Some(Filter::User(user)) => policy_user(user),
        None => String::from(POLICY_DEFAULT),
    };

    file.write_all(content.as_bytes())?;

    Ok(())
}

fn policy_group(group: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR(FILE_SIZE))
  WHERE GROUP_ID = {group}
"
    )
}

fn policy_user(user: &str) -> String {
    format!(
        "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR(FILE_SIZE))
  WHERE USER_ID = {user}
"
    )
}

const POLICY_DEFAULT: &str = "RULE
  EXTERNAL LIST 'size'
  EXEC ''

RULE 'TOTAL'
  LIST 'size'
  DIRECTORIES_PLUS
  SHOW(VARCHAR(FILE_SIZE))
";
