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

use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
#[cfg(feature = "log")]
use bstr::ByteSlice;
use bstr::io::BufReadExt;
use clap::crate_version;

use crate::policy::NcduEntry;

pub fn sum(root: &Path, report: &mut impl Read) -> Result<FSTree> {
    let report = BufReader::new(report);

    let mut fs_tree =
        FSTree(root.to_owned(), Data::default(), BTreeMap::new());

    for line in report.byte_lines() {
        let line = line.context("reading line from policy report")?;

        #[cfg(feature = "log")]
        log::trace!("line: {:?}", line.to_str_lossy());

        let entry = NcduEntry::try_from(&line)
            .context("parsing line from policy report")?;

        fs_tree.insert(&entry)?;
    }

    Ok(fs_tree)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Data {
    file_size: u64,
    kb_allocated: u64,
    nlink: u32,
    inode: u64,
}

impl Data {
    fn write(&self, output: &mut impl Write) -> Result<()> {
        if self.file_size != 0 {
            write!(output, r#","asize":{}"#, self.file_size)?;
        }

        if self.kb_allocated != 0 {
            write!(output, r#","dsize":{}"#, self.kb_allocated * 1024)?;
        }

        if self.nlink > 1 {
            write!(output, r#","nlink":{}"#, self.nlink)?;
            write!(output, r#","ino":{}"#, self.inode)?;
        }

        Ok(())
    }
}

impl TryFrom<&NcduEntry<'_>> for Data {
    type Error = anyhow::Error;

    fn try_from(entry: &NcduEntry) -> Result<Self> {
        let file_size = entry.file_size()?;
        let kb_allocated = entry.kb_allocated()?;
        let nlink = entry.nlink()?;
        let inode = if nlink > 1 { entry.inode()? } else { 0 };

        let value = Self {
            file_size,
            kb_allocated,
            nlink,
            inode,
        };

        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FSObj {
    Dir(FSTree),
    Node(Data),
}

impl FSObj {
    fn insert(&mut self, entry: &NcduEntry) -> Result<()> {
        match self {
            Self::Dir(subtree) => subtree.insert(entry),
            Self::Node(_) => Err(anyhow!("insert into node {self:?}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FSTree(PathBuf, Data, BTreeMap<PathBuf, FSObj>);

impl FSTree {
    fn path(&self) -> &Path {
        self.0.as_path()
    }

    const fn data(&self) -> Data {
        self.1
    }

    const fn data_mut(&mut self) -> &mut Data {
        &mut self.1
    }

    const fn tree(&self) -> &BTreeMap<PathBuf, FSObj> {
        &self.2
    }

    const fn tree_mut(&mut self) -> &mut BTreeMap<PathBuf, FSObj> {
        &mut self.2
    }

    fn insert(&mut self, entry: &NcduEntry) -> Result<()> {
        let path = entry.path()?;

        #[cfg(feature = "log")]
        log::trace!(
            "inserting {} into tree of {}",
            path.display(),
            self.path().display()
        );

        let data = Data::try_from(entry)?;

        if self.path() == path {
            #[cfg(feature = "log")]
            log::trace!("UPDATE data of {}", path.display());

            *self.data_mut() = data;

            return Ok(());
        }

        let parent = path.parent().with_context(|| {
            format!("path has no parent: {}", path.display())
        })?;

        if self.path() == parent {
            // we are correct in current node
            let key = path.to_owned();

            let value: FSObj = if entry.mode_str()?.starts_with('d') {
                FSObj::Dir(Self(key.clone(), data, BTreeMap::new()))
            } else {
                FSObj::Node(data)
            };

            self.tree_mut()
                .entry(key)
                .and_modify(|value| match value {
                    FSObj::Dir(tree) => {
                        #[cfg(feature = "log")]
                        log::trace!("UPDATE data of {}", path.display());

                        *tree.data_mut() = data;
                    }

                    FSObj::Node(node_data) => {
                        #[cfg(feature = "log")]
                        log::warn!(
                            "discovered node {} twice, overriding data",
                            path.display()
                        );
                        *node_data = data;
                    }
                })
                .or_insert_with(|| {
                    #[cfg(feature = "log")]
                    log::trace!(
                        "INSERT {} -> {:?} into tree of {}",
                        path.display(),
                        value,
                        parent.display(),
                    );

                    value
                });
        } else {
            // descend into subdirs
            let current_len = self.path().ancestors().count();
            let key: PathBuf = path.iter().take(current_len + 1).collect();

            self.tree_mut()
                .entry(key.clone())
                .or_insert_with(|| {
                    #[cfg(feature = "log")]
                    log::trace!(
                        "CREATE new empty subtree {:?}",
                        key.display()
                    );

                    FSObj::Dir(Self(key, Data::default(), BTreeMap::new()))
                })
                .insert(entry)?;
        }

        Ok(())
    }

    pub fn write(&self, output: &mut impl Write) -> Result<()> {
        write!(
            output,
            r#"[1,2,{{"progname":"mmdu","progver":"{}"}}"#,
            crate_version!(),
        )?;

        self.write_tree(output, true)?;

        writeln!(output, "]")?;

        Ok(())
    }

    fn write_tree(&self, output: &mut impl Write, root: bool) -> Result<()> {
        writeln!(output, ",")?;
        write!(output, r#"[{{"name":""#)?;

        if root {
            write!(output, "{}", self.path().display())?;
        } else {
            let dir_name = self.path().file_name().with_context(|| {
                format!("path has no file name: {}", self.path().display())
            })?;

            output.write_all(dir_name.as_encoded_bytes())?;
        }

        write!(output, r#"""#)?;
        self.data().write(output)?;
        write!(output, "}}")?;

        for (path, node) in self.tree() {
            match node {
                FSObj::Dir(tree) => tree.write_tree(output, false)?,
                FSObj::Node(data) => {
                    let file_name = path.file_name().with_context(|| {
                        format!("path has no file name: {}", path.display())
                    })?;

                    writeln!(output, ",")?;
                    write!(output, r#"{{"name":""#)?;
                    output.write_all(file_name.as_encoded_bytes())?;
                    write!(output, r#"""#)?;
                    data.write(output)?;
                    write!(output, "}}")?;
                }
            }
        }

        write!(output, "]")?;

        Ok(())
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use indoc::{formatdoc, indoc};

    use super::*;

    #[allow(clippy::missing_const_for_fn)]
    fn init() {
        #[cfg(feature = "log")]
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn parse() {
        init();

        let source = indoc! {"
            1 0 0  drwx------ 1 4096 0 -- /data/test
            2 0 0  drwxr-xr-x 1 4096 0 -- /data/test/a
            3 0 0  -rw-r--r-- 1 1024 0 -- /data/test/a/baz
            4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/bar
            4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/foo
            4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/a/foo
            4 0 0  -rw-r--r-- 4 1024 0 -- /data/test/b/bar
            5 0 0  drwxr-xr-x 1 4096 0 -- /data/test/b
        "};

        let result =
            sum(Path::new("/data/test"), &mut source.as_bytes()).unwrap();

        let mut a = BTreeMap::new();
        a.insert(
            "/data/test/a/baz".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            }),
        );
        a.insert(
            "/data/test/a/foo".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 4,
            }),
        );
        let a = FSTree(
            "/data/test/a".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            },
            a,
        );

        let mut b = BTreeMap::new();
        b.insert(
            "/data/test/b/bar".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 4,
            }),
        );
        let b = FSTree(
            "/data/test/b".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            },
            b,
        );

        let mut root = BTreeMap::new();
        root.insert(
            "/data/test/bar".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 4,
            }),
        );
        root.insert(
            "/data/test/foo".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 4,
            }),
        );
        root.insert("/data/test/a".into(), FSObj::Dir(a));
        root.insert("/data/test/b".into(), FSObj::Dir(b));
        let expected = FSTree(
            "/data/test".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            },
            root,
        );

        assert_eq!(expected, result);
    }

    #[test]
    fn write_empty() {
        init();

        let tree = FSTree(
            "/data/test".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            },
            BTreeMap::new(),
        );

        let mut result: Vec<u8> = Vec::new();
        tree.write(&mut result).unwrap();
        let result = std::str::from_utf8(&result).unwrap();

        let expected = formatdoc! {r#"
            [1,2,{{"progname":"mmdu","progver":"{progver}"}},
            [{{"name":"/data/test","asize":4096}}]]
            "#,
            progver = crate_version!(),
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn write_flat() {
        init();

        let mut root = BTreeMap::new();
        root.insert(
            "/data/test/bar".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 2,
                inode: 2,
            }),
        );
        root.insert(
            "/data/test/foo".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 2,
                inode: 2,
            }),
        );

        let tree = FSTree(
            "/data/test".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 1,
            },
            root,
        );

        let mut result: Vec<u8> = Vec::new();
        tree.write(&mut result).unwrap();
        let result = std::str::from_utf8(&result).unwrap();

        let expected = formatdoc! {r#"
            [1,2,{{"progname":"mmdu","progver":"{progver}"}},
            [{{"name":"/data/test","asize":4096}},
            {{"name":"bar","asize":1024,"nlink":2,"ino":2}},
            {{"name":"foo","asize":1024,"nlink":2,"ino":2}}]]
            "#,
            progver = crate_version!(),
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn write_nested() {
        init();

        let mut a = BTreeMap::new();
        a.insert(
            "/data/test/a/baz".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 1,
                inode: 4,
            }),
        );
        a.insert(
            "/data/test/a/foo".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 1,
            }),
        );
        let a = FSTree(
            "/data/test/a".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 3,
            },
            a,
        );

        let mut b = BTreeMap::new();
        b.insert(
            "/data/test/b/bar".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 1,
            }),
        );
        let b = FSTree(
            "/data/test/b".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 2,
            },
            b,
        );

        let mut root = BTreeMap::new();
        root.insert(
            "/data/test/bar".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 1,
            }),
        );
        root.insert(
            "/data/test/foo".into(),
            FSObj::Node(Data {
                file_size: 1024,
                kb_allocated: 0,
                nlink: 4,
                inode: 1,
            }),
        );
        root.insert("/data/test/a".into(), FSObj::Dir(a));
        root.insert("/data/test/b".into(), FSObj::Dir(b));
        let tree = FSTree(
            "/data/test".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 1,
            },
            root,
        );

        let mut result: Vec<u8> = Vec::new();
        tree.write(&mut result).unwrap();
        let result = std::str::from_utf8(&result).unwrap();

        let expected = formatdoc! {r#"
            [1,2,{{"progname":"mmdu","progver":"{progver}"}},
            [{{"name":"/data/test","asize":4096}},
            [{{"name":"a","asize":4096}},
            {{"name":"baz","asize":1024}},
            {{"name":"foo","asize":1024,"nlink":4,"ino":1}}],
            [{{"name":"b","asize":4096}},
            {{"name":"bar","asize":1024,"nlink":4,"ino":1}}],
            {{"name":"bar","asize":1024,"nlink":4,"ino":1}},
            {{"name":"foo","asize":1024,"nlink":4,"ino":1}}]]
            "#,
            progver = crate_version!(),
        };

        assert_eq!(expected, result);
    }
}
