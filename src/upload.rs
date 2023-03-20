use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::eyre::WrapErr;
use google_drive3::{hyper, hyper_rustls, oauth2, oauth2::InstalledFlowReturnMethod, DriveHub};
use tokio::fs;
use tracing::instrument;

use crate::{storage::Redis, Claim, Config};

#[instrument(skip(config, redis))]
pub async fn upload_files(
    claim: Claim,
    files: &[Utf8PathBuf],
    upload_path: &Utf8Path,
    config: Arc<Config>,
    redis: Arc<Redis>,
) -> color_eyre::Result<()> {
    let auth = oauth2::InstalledFlowAuthenticator::builder(
        config.google_credentials.clone(),
        InstalledFlowReturnMethod::HTTPPortRedirect(12346),
    )
    .with_storage(Box::new(redis.get_storage(claim.token_id)))
    .build()
    .await
    .wrap_err("failed to build authenticator")?;

    let hub = DriveHub::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
        ),
        auth,
    );
    for file in files {
        let upload_file_name = file
            .file_name()
            .map(|p| upload_path.join(p))
            .and_then(|p| p.file_name().map(String::from));
        let google_file = google_drive3::api::File {
            name: upload_file_name,
            ..Default::default()
        };
        let f = fs::File::open(file).await?;
        let c = hub.files().create(google_file);
        let _ = c
            .upload(f.into_std().await, guess_mime_from_file(file))
            .await
            .wrap_err("failed to upload file")?;
    }
    Ok(())
}

pub fn guess_mime_from_file(file: &Utf8Path) -> mime::Mime {
    match file.extension() {
        Some("pdf") => mime::APPLICATION_PDF,
        Some("txt") => mime::TEXT_PLAIN,
        _ => mime::APPLICATION_OCTET_STREAM,
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use test_case::test_case;

    #[test_case("a.pdf" => mime::APPLICATION_PDF)]
    #[test_case("a.txt" => mime::TEXT_PLAIN)]
    #[test_case("a.ogg" => mime::APPLICATION_OCTET_STREAM)]
    fn guess_mime_from_file(input: &str) -> mime::Mime {
        super::guess_mime_from_file(&Utf8PathBuf::from(input))
    }
}
