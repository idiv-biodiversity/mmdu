mmdu
====

Disk usage for [IBM Spectrum Scale][] (formerly GPFS) file systems.

-   uses `mmapplypolicy` instead of universal directory traversal

    This can be considerably faster, especially for large directories, because
    it uses file system internals and can use extensive parallelism. See the
    respective command-line options in `--help` output for more information.

-   aims to be CLI compatible with `du` from [coreutils][]

    Not all options of `du` are supported yet. Some defaults are still
    different.

Run with `-?` for concise help and `--help` for full help.


Motivation and Usage
--------------------

The main motivation is a speedy alternative to `du` due to `mmapplypolicy`
being much faster then universal directory traversal.

But because `mmapplypolicy` can not be run as a regular user, it is advisable
to set up timer or cron jobs to run this as a service for users and groups. You
could set up timers that run `mmdu --max-depth x` for each `/data/dir` and save
the output to `/data/dir/disk-usage.txt`. Users could configure the depth and
then check the output with `sort -h /data/dir/disk-usage.txt`. This avoids that
users have to do their own slow-running `du -sh` and avoids that stress to the
file systems.


Installation
------------

### cargo install

```bash
cargo install mmdu
```

### from source

```bash
git clone https://github.com/idiv-biodiversity/mmdu.git
cd mmdu
cargo build --release
install -Dm755 target/release/mmdu ~/bin/mmdu
```


[IBM Spectrum Scale]: https://www.ibm.com/products/spectrum-scale
[coreutils]: https://www.gnu.org/software/coreutils/
