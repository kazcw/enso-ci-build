use std::vec::IntoIter;
// use std::vec::IntoIter;
use crate::prelude::*;

use crate::archive::Format;


#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum Compression {
    Bzip2,
    Gzip,
    Lzma,
    Xz,
}

impl Compression {
    pub fn deduce_from_extension(extension: impl AsRef<Path>) -> Result<Compression> {
        let extension = extension.as_ref().to_str().unwrap();
        if extension == "bz2" {
            Ok(Compression::Bzip2)
        } else if extension == "gz" {
            Ok(Compression::Gzip)
        } else if extension == "lzma" {
            Ok(Compression::Lzma)
        } else if extension == "xz" {
            Ok(Compression::Xz)
        } else {
            bail!("The extension `{}` does not denote a supported compression algorithm for TAR archives.", extension)
        }
    }
}

impl Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Compression::*;
        write!(f, "{}", match self {
            Bzip2 => "bzip2",
            Gzip => "gzip",
            Lzma => "lzma",
            Xz => "xz",
        })
    }
}

impl AsRef<str> for Compression {
    fn as_ref(&self) -> &str {
        match self {
            Compression::Bzip2 => "-j",
            Compression::Gzip => "-z",
            Compression::Lzma => "--lzma",
            Compression::Xz => "-J",
        }
    }
}

impl AsRef<OsStr> for Compression {
    fn as_ref(&self) -> &OsStr {
        let str: &str = self.as_ref();
        str.as_ref()
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum Switch {
    TargetFile(PathBuf),
    Verbose,
    UseFormat(Compression),
    WorkingDir(PathBuf),
}

impl<'a> IntoIterator for &'a Switch {
    type Item = &'a OsStr;
    type IntoIter = IntoIter<&'a OsStr>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Switch::TargetFile(tgt) => vec!["-f".as_ref(), tgt.as_ref()],
            Switch::Verbose => vec!["--verbose".as_ref()],
            Switch::UseFormat(compression) => vec![compression.as_ref()],
            Switch::WorkingDir(dir) => vec!["--directory".as_ref(), dir.as_ref()],
        }
        .into_iter()
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum Command {
    Append,
    Create,
    Extract,
    List,
}

impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        match self {
            Command::Append => "-r",
            Command::Create => "-c",
            Command::Extract => "-x",
            Command::List => "-t",
        }
    }
}

impl AsRef<OsStr> for Command {
    fn as_ref(&self) -> &OsStr {
        let str: &str = self.as_ref();
        str.as_ref()
    }
}

pub struct Tar;

impl Program for Tar {
    fn executable_name() -> &'static str {
        "tar"
    }
}

impl Tar {
    pub fn pack_cmd<P: AsRef<Path>>(
        &self,
        output_archive: impl AsRef<Path>,
        paths_to_pack: impl IntoIterator<Item = P>,
    ) -> Result<crate::prelude::Command> {
        let mut cmd = self.cmd()?;
        cmd.arg(Command::Create);

        if let Ok(Format::Tar(Some(compression))) = Format::from_filename(&output_archive) {
            cmd.args(&Switch::UseFormat(compression));
        }

        cmd.args(&Switch::TargetFile(output_archive.as_ref().into()));

        let paths: Vec<PathBuf> =
            paths_to_pack.into_iter().map(|path| path.as_ref().to_owned()).collect();

        match paths.as_slice() {
            [item] =>
                if let Some(parent) = item.canonicalize()?.parent() {
                    cmd.args(&Switch::WorkingDir(parent.to_owned()));
                    cmd.arg(item.file_name().unwrap()); // None can happen only when path ends with
                                                        // ".." - that's why we canonicalize
                },
            // [dir] if dir.is_dir() => {
            //     cmd.args(&Switch::WorkingDir(dir.to_owned()));
            //     cmd.arg(".");
            // }
            _ => {
                todo!("")
            } /* paths => {
               *     if let Some(parent) = output_archive.as_ref().parent() {
               *         cmd.arg(Switch::WorkingDir(parent.to_owned()).format_arguments());
               *         for path_to_pack in paths {
               *             if path_to_pack.is_absolute() {
               *                 pathdiff::diff_paths(parent, path_to_pack).ok_or_else(||
               * anyhow!("failed to relativize paths {} {}", parent, path_to_pack))
               *             }
               *             cmd.arg(&path_to_pack);
               *         },
               *     }
               * } */
        }


        Ok(cmd)
        // cmd_from_args![Command::Create, val [switches], output_archive.as_ref(), ref
        // [paths_to_pack]]
    }

    pub async fn pack<P: AsRef<Path>>(
        self,
        output_archive: impl AsRef<Path>,
        paths_to_pack: impl IntoIterator<Item = P>,
    ) -> Result {
        self.pack_cmd(output_archive, paths_to_pack)?.run_ok().await
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn deduce_format_from_extension() {
        let expect_ok = |str: &str, expected: Compression| {
            assert_eq!(Compression::deduce_from_extension(&OsStr::new(str)).unwrap(), expected);
        };

        expect_ok("bz2", Compression::Bzip2);
        expect_ok("gz", Compression::Gzip);
        expect_ok("lzma", Compression::Lzma);
        expect_ok("xz", Compression::Xz);
    }

    #[test]
    fn pack_command_test() {
        let cmd = Tar.pack_cmd("output.tar.gz", &["target.bmp"]).unwrap();
        println!("{:?}", cmd);
        dbg!(cmd);
    }
}