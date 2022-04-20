use crate::prelude::*;

use crate::fs::create_dir_if_missing;
use crate::programs;
use crate::programs::tar::Compression;
use crate::programs::tar::Tar;
use crate::programs::SevenZip;

use tracing::Span;

pub mod tar;
pub mod zip;

/// Archive formats that we handle.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Format {
    Zip,
    SevenZip,
    Tar(Option<programs::tar::Compression>),
}

impl Format {
    /// Deduce the archive format from a given filename.
    #[context("Deducing archive format from a filename {}.", filename.as_ref().display())]
    pub fn from_filename(filename: impl AsRef<Path>) -> Result<Self> {
        let filename = filename.as_ref();
        let extension =
            filename.extension().ok_or_else(|| anyhow!("The path had no extension."))?;
        match extension.to_str().unwrap() {
            "zip" => Ok(Format::Zip),
            "7z" => Ok(Format::SevenZip),
            "tgz" => Ok(Format::Tar(Some(programs::tar::Compression::Gzip))),
            "txz" => Ok(Format::Tar(Some(programs::tar::Compression::Xz))),
            other => {
                if let Ok(compression) = programs::tar::Compression::deduce_from_extension(other) {
                    let secondary_extension =
                        filename.file_stem().map(Path::new).and_then(Path::extension);
                    if secondary_extension == Some(OsStr::new("tar")) {
                        Ok(Format::Tar(Some(compression)))
                    } else {
                        bail!("Extension `.{}` looks like a tar compression, but there is no `.tar.` component in the name", other)
                    }
                } else {
                    bail!("Unrecognized archive extension `{}`.", other)
                }
            }
        }
    }

    /// Extract an archive of this format into a given output directory.
    pub fn extract(
        self,
        compressed_data: impl Read + Seek,
        output_dir: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let _bar = crate::global::new_spinner(format!(
            "Unpacking archive to {}",
            output_dir.as_ref().display()
        ));
        create_dir_if_missing(&output_dir)?;
        match self {
            Format::Zip => {
                let mut archive = zip::ZipArchive::new(compressed_data)?;
                archive.extract(output_dir)?;
            }
            Format::Tar(Some(Compression::Gzip)) => {
                let tar_stream = flate2::read::GzDecoder::new(compressed_data);
                let mut archive = ::tar::Archive::new(tar_stream);
                archive.unpack(output_dir)?;
            }
            // Format::SevenZip => {
            //     let mut cmd = SevenZip.unpack_from_stdin_cmd(output_dir)?;
            //     cmd.stdin(Stdio::piped());
            //     let mut child = cmd.as_std().clone().spawn()?;
            //     //let child = cmd.spawn_nicer()?;
            //     let mut stdin =
            //         child.stdin.ok_or_else(|| anyhow!("Failed to get 7z stdin handle"))?;
            //     std::io::copy(&mut compressed_data, &mut stdin)?;
            //     drop(stdin);
            //     child.wait()?.exit_ok()?;
            // }
            _ => todo!("Not supported!"),
        }
        Ok(())
    }
}


pub async fn create(
    output_archive: impl AsRef<Path>,
    paths_to_pack: impl IntoIterator<Item: AsRef<Path>>,
) -> Result {
    let span = info_span!("Creating an archive", target = output_archive.as_ref().as_str());
    let format = Format::from_filename(&output_archive)?;
    match format {
        Format::Zip | Format::SevenZip =>
            SevenZip.pack(output_archive, paths_to_pack).instrument(span).await,
        Format::Tar(_) => Tar.pack(output_archive, paths_to_pack).instrument(span).await,
    }
}

pub async fn pack_directory_contents(
    output_archive: impl AsRef<Path>,
    root_directory: impl AsRef<Path>,
) -> Result {
    let span = info_span!(
        "Creating an archive",
        source = root_directory.as_ref().as_str(),
        target = output_archive.as_ref().as_str()
    );
    let format = Format::from_filename(&output_archive)?;
    match format {
        Format::Zip | Format::SevenZip =>
            SevenZip.pack_directory_contents(output_archive, root_directory).instrument(span).await,
        Format::Tar(_) =>
            Tar.pack_directory_contents(output_archive, root_directory).instrument(span).await,
    }
}

#[tracing::instrument(
    name="Extracting item from archive.",
    skip(archive_path, item_path, output_path),
    fields(
        src=%archive_path.as_ref().display(),
        item=%item_path.as_ref().display(),
        dest=%output_path.as_ref().display()),
    err)]
pub async fn extract_item(
    archive_path: impl AsRef<Path>,
    item_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result {
    let format = Format::from_filename(&archive_path)?;
    let archive_path = archive_path.as_ref().to_path_buf();
    let item_path = item_path.as_ref().to_path_buf();
    let output_path = output_path.as_ref().to_path_buf();

    let extract_task = match format {
        Format::Zip => {
            let mut archive = zip::open(&archive_path)?;
            tokio::task::spawn_blocking(move || {
                zip::extract_subtree(&mut archive, item_path, output_path)
            })
        }
        Format::Tar(Some(Compression::Gzip)) => {
            let mut archive = tar::open_tar_gz(&archive_path)?;
            tokio::task::spawn_blocking(move || {
                tar::extract_subtree(&mut archive, item_path, output_path)
            })
        }
        _ => todo!(),
    };
    extract_task.instrument(Span::current()).await??;
    Ok(())
}

#[tracing::instrument(name="Extracting the archive to a directory.", skip(archive_path,output_directory), fields(src=%archive_path.as_ref().display(), dest=%output_directory.as_ref().display()), err)]
pub async fn extract_to(
    archive_path: impl AsRef<Path>,
    output_directory: impl AsRef<Path>,
) -> Result {
    // Don't clean the output directory. Perhaps even the archive lives there.
    let span = info_span!(
        "Extracting the archive.",
        source = archive_path.as_ref().as_str(),
        target = output_directory.as_ref().as_str()
    );
    let format = Format::from_filename(&archive_path)?;
    match format {
        Format::Zip | Format::SevenZip =>
            SevenZip.unpack_cmd(archive_path, output_directory)?.run_ok().instrument(span).await,
        Format::Tar(_) => Tar.unpack(archive_path, output_directory).instrument(span).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_filename() -> Result {
        assert_eq!(
            Format::from_filename("/tmp/.tmpnejBKd/gui_wasm.tar.gz")?,
            Format::Tar(Some(Compression::Gzip))
        );
        Ok(())
    }
}
