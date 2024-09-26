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
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use bstr::io::BufReadExt;
#[cfg(feature = "log")]
use bstr::ByteSlice;
use clap::crate_version;

use crate::config::{ByteMode, Config};
use crate::policy::NcduEntry;
use crate::usage::depth::DepthAcc;

use super::Acc;

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
    fn sum_total(
        &self,
        acc: &mut Acc,
        hard_links: &mut Option<HashMap<u64, u64>>,
        byte_mode: ByteMode,
    ) {
        let value = match byte_mode {
            ByteMode::FileSize => self.file_size,
            ByteMode::KBAllocated => self.kb_allocated,
        };

        // early return if there is only one link
        if self.nlink == 1 {
            *acc += value;
            return;
        }

        if let Some(hard_links) = hard_links {
            let inode = hard_links
                .entry(self.inode)
                .and_modify(|c| *c += 1)
                .or_insert(1);

            if *inode == 1 {
                *acc += value;
            }
        } else {
            *acc += value;
        }
    }

    fn sum_depth(
        &self,
        sums: &mut HashMap<PathBuf, DepthAcc>,
        path: &Path,
        prefix_depth: usize,
        max_depth: usize,
        config: &Config,
    ) {
        let path_depth = path.iter().count();
        let path_suffix_depth = path_depth - prefix_depth;

        for depth in 0..=max_depth.min(path_suffix_depth) {
            let prefix: PathBuf =
                path.iter().take(prefix_depth + depth).collect();

            // sums.entry(prefix).and_modify(|acc| {
            //     self.sum_total(&mut acc.acc, &mut acc.hard_links, byte_mode)
            // });
        }
    }

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

    fn data_mut(&mut self) -> &mut Data {
        &mut self.1
    }

    const fn tree(&self) -> &BTreeMap<PathBuf, FSObj> {
        &self.2
    }

    fn tree_mut(&mut self) -> &mut BTreeMap<PathBuf, FSObj> {
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

    pub fn to_total(&self, config: &Config) -> Acc {
        let mut acc = Acc::default();
        let mut hard_links = config.count_links.then(HashMap::new);
        self.sum_total_rec(&mut acc, &mut hard_links, config.byte_mode);
        acc
    }

    fn sum_total_rec(
        &self,
        acc: &mut Acc,
        hard_links: &mut Option<HashMap<u64, u64>>,
        byte_mode: ByteMode,
    ) {
        self.data().sum_total(acc, hard_links, byte_mode);

        for fsobj in self.tree().values() {
            match fsobj {
                FSObj::Dir(tree) => {
                    tree.sum_total_rec(acc, hard_links, byte_mode);
                }

                FSObj::Node(data) => {
                    data.sum_total(acc, hard_links, byte_mode);
                }
            }
        }
    }

    pub fn to_depth(
        &self,
        dir: &Path,
        max_depth: usize,
        config: &Config,
    ) -> BTreeMap<PathBuf, Acc> {
        let mut sums: HashMap<PathBuf, DepthAcc> = HashMap::new();
        let prefix_depth = dir.iter().count();

        self.sum_depth_rec(&mut sums, prefix_depth, max_depth, config);

        sums.into_iter()
            .filter_map(|(path, value)| {
                (value.acc.inodes > 1).then_some((path, value.acc))
            })
            .collect()
    }

    fn sum_depth_rec(
        &self,
        sums: &mut HashMap<PathBuf, DepthAcc>,
        prefix_depth: usize,
        max_depth: usize,
        config: &Config,
    ) {
        self.data().sum_depth(
            sums,
            self.path(),
            prefix_depth,
            max_depth,
            config,
        );

        for fsobj in self.tree().values() {
            match fsobj {
                FSObj::Dir(tree) => {
                    tree.sum_depth_rec(sums, prefix_depth, max_depth, config);
                }

                FSObj::Node(data) => {
                    data.sum_depth(
                        sums,
                        self.path(),
                        prefix_depth,
                        max_depth,
                        config,
                    );
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------
// tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use indoc::formatdoc;

    use super::*;

    // ALLOW const is only possible when feature disabled
    #[allow(clippy::missing_const_for_fn)]
    fn init() {
        #[cfg(feature = "log")]
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn example_tree() -> FSTree {
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

        FSTree(
            "/data/test".into(),
            Data {
                file_size: 4096,
                kb_allocated: 0,
                nlink: 1,
                inode: 0,
            },
            root,
        )
    }

    #[test]
    fn parse() {
        init();

        let source = &mut NcduEntry::EXAMPLE.as_bytes();
        let result = sum(Path::new("/data/test"), source).unwrap();

        let expected = example_tree();

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

        let tree = example_tree();

        let mut result: Vec<u8> = Vec::new();
        tree.write(&mut result).unwrap();
        let result = std::str::from_utf8(&result).unwrap();

        let expected = formatdoc! {r#"
            [1,2,{{"progname":"mmdu","progver":"{progver}"}},
            [{{"name":"/data/test","asize":4096}},
            [{{"name":"a","asize":4096}},
            {{"name":"baz","asize":1024}},
            {{"name":"foo","asize":1024,"nlink":4,"ino":4}}],
            [{{"name":"b","asize":4096}},
            {{"name":"bar","asize":1024,"nlink":4,"ino":4}}],
            {{"name":"bar","asize":1024,"nlink":4,"ino":4}},
            {{"name":"foo","asize":1024,"nlink":4,"ino":4}}]]
            "#,
            progver = crate_version!(),
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn ncdu_to_total() {
        init();

        let tree = example_tree();

        let mut hard_links_not_counted = Acc::default();
        tree.sum_total_rec(
            &mut hard_links_not_counted,
            &mut Some(HashMap::new()),
            ByteMode::FileSize,
        );

        assert_eq!(Acc::from((5, 14336)), hard_links_not_counted);

        let mut hard_links_counted = Acc::default();
        tree.sum_total_rec(
            &mut hard_links_counted,
            &mut None,
            ByteMode::FileSize,
        );

        assert_eq!(Acc::from((8, 17408)), hard_links_counted);
    }

    #[test]
    fn ncdu_to_depth() {
        init();

        let config = Config {
            byte_mode: ByteMode::FileSize,
            count_links: false,
            ..Default::default()
        };

        let tree = example_tree();

        let mut expected_zero = BTreeMap::new();
        expected_zero.insert("/data/test".into(), Acc::from((5, 14336)));

        assert_eq!(
            expected_zero,
            tree.to_depth(Path::new("/data/test"), 0, &config)
        );

        let mut expected_one = BTreeMap::new();
        expected_one.insert("/data/test".into(), Acc::from((5, 14336)));
        expected_one.insert("/data/test/a".into(), Acc::from((3, 6144)));
        expected_one.insert("/data/test/b".into(), Acc::from((2, 5120)));

        assert_eq!(
            expected_one,
            tree.to_depth(Path::new("/data/test"), 1, &config)
        );
    }
}
