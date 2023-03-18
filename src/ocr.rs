use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    eyre::{eyre, WrapErr},
    Result, Section, SectionExt,
};
use futures_util::StreamExt;
use lazy_static::lazy_static;
use regex::Regex;
use tokio::{fs, fs::File, io::AsyncWriteExt, process::Command, spawn};
use tracing::{info, info_span, instrument, Instrument};

use crate::Payload;

lazy_static! {
    pub static ref LANGUAGE_REGEX: Regex =
        Regex::new(r"\.([a-z]{3})\.pdf$").expect("invalid regex");
}

#[instrument(skip_all, fields(filename=payload.filename, path=?payload.path))]
pub async fn process_input(payload: &Payload) -> Result<Vec<Utf8PathBuf>> {
    let working_dir = spawn(async { tempfile::TempDir::new() })
        .await?
        .wrap_err("failed to create temporary directory")?
        .into_path();
    let working_dir =
        Utf8PathBuf::from_path_buf(working_dir).expect("invalid temporary dir created");
    let origin_file_path = working_dir.join(&payload.filename);
    let response = reqwest::get(payload.file_url.clone())
        .await
        .wrap_err("failed to download file")?;
    let mut written_size = 0;

    {
        let mut origin_file = File::create(&origin_file_path)
            .await
            .wrap_err("failed to create local file")?;
        let mut input = response.bytes_stream();

        while let Some(bytes) = input.next().await {
            let bytes = bytes?;
            origin_file.write_all(&bytes).await?;
            written_size += bytes.len();
        }
    }

    info!(?origin_file_path, written_size, "Downloaded pdf");

    let output_path = &working_dir.join("ocr");
    fs::create_dir(output_path).await?;
    process_file(output_path, &origin_file_path)
        .await
        .wrap_err("failed to process file")
}

#[instrument(ret)]
async fn process_file(output_path: &Utf8Path, pdf_path: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let original_filename = pdf_path.file_name().unwrap();
    let ocred_pdf = output_path.join(original_filename);
    let sidecar_file = Utf8PathBuf::from(original_filename).with_extension("txt");
    let sidecar_file = output_path.join(sidecar_file);
    let language = get_language_from_file(pdf_path).unwrap_or_else(|| "eng".to_string());
    let arguments = [
        "-l",
        language.as_str(),
        "--force-ocr",
        pdf_path.as_str(),
        "--sidecar",
        sidecar_file.as_str(),
        ocred_pdf.as_str(),
    ];
    let mut command = Command::new("ocrmypdf");
    command.args(arguments);
    let output = command
        .output()
        .instrument(info_span!("ocrmypdf", ?arguments))
        .await
        .wrap_err("failed call spawn ocrmypdf")?;

    if output.status.success() {
        return Ok(vec![ocred_pdf, sidecar_file]);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(eyre!("ocrymypdf failed").with_section(|| stderr.trim().to_string().header("Stderr:")))
}

fn get_language_from_file(path: &Utf8Path) -> Option<String> {
    path.file_name()
        .and_then(|path| LANGUAGE_REGEX.captures(path))
        .and_then(|p| p.get(1).map(|f| f.as_str()))
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use color_eyre::Result;
    use tempfile::tempdir;
    use test_case::test_case;
    use tokio::fs;

    use crate::ocr::{get_language_from_file, process_file};

    #[test_case("german.deu.pdf" => Some("deu".to_string()))]
    #[test_case("english.eng.pdf" => Some("eng".to_string()))]
    #[test_case("non_matching.pdf" => None)]
    #[test_case("portuguese.pob.pdf" => Some("pob".to_string()))]
    pub fn get_language(language: &str) -> Option<String> {
        let file = Utf8PathBuf::from(language);
        get_language_from_file(&file)
    }

    #[tokio::test]
    async fn test_ocr_pdf() -> Result<()> {
        color_eyre::install().ok();
        let working_dir = Utf8PathBuf::from_path_buf(tempdir()?.into_path()).unwrap();
        let test_pdf = Utf8PathBuf::from("fixtures/test.pdf");

        let mut files = process_file(&working_dir, test_pdf.as_path())
            .await?
            .into_iter();
        let pdf = files.next().unwrap();
        let txt = files.next().unwrap();
        assert!(pdf.exists());
        assert!(txt.exists());

        let ocred = fs::read_to_string(txt).await?.replace("\n", "");
        let expected = fs::read_to_string("fixtures/expected.txt")
            .await?
            .replace("\n", "");
        assert_eq!(ocred, expected);
        Ok(())
    }
}
