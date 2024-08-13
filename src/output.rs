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

use bytesize::ByteSize;

use crate::config::{ByteMode, Config, CountMode};

pub fn output(dir: &Path, inodes: u64, bytes: u64, config: &Config) {
    let humanized = match config.byte_mode {
        ByteMode::FileSize => humanize(ByteSize::b(bytes)),
        ByteMode::KBAllocated => humanize(ByteSize::kib(bytes)),
    };

    let dir = dir.display();

    match config.count_mode {
        CountMode::Both => println!("{humanized}\t{inodes}\t{dir}"),
        CountMode::Bytes => println!("{humanized}\t{dir}"),
        CountMode::Inodes => println!("{inodes}\t{dir}"),
    }
}

fn humanize(bytes: ByteSize) -> String {
    bytes
        .to_string_as(true)
        .replace("iB", "")
        .replace(' ', "")
        .to_uppercase()
}
