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

use std::path::Path;

use bytesize::ByteSize;

use crate::config::Config;

pub fn output(dir: &Path, inodes: u64, bytes: u64, config: &Config) {
    let humanized = humanize(bytes);
    let dir = dir.display();

    if config.count_bytes && config.count_inodes {
        println!("{humanized}\t{inodes}\t{dir}");
    } else if config.count_bytes {
        println!("{humanized}\t{dir}");
    } else if config.count_inodes {
        println!("{inodes}\t{dir}");
    }
}

fn humanize(bytes: u64) -> String {
    ByteSize(bytes)
        .to_string_as(true)
        .replace("iB", "")
        .replace(' ', "")
        .to_uppercase()
}
