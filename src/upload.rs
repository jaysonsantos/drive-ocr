use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    eyre::{eyre, WrapErr},
    Result,
};
use google_drive3::{
    hyper, hyper::client::HttpConnector, hyper_rustls, hyper_rustls::HttpsConnector, oauth2,
    oauth2::InstalledFlowReturnMethod, DriveHub,
};
use tokio::fs;
use tracing::{info, info_span, instrument, Instrument};

use crate::{storage::Redis, Claim, Config};

const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";

#[instrument(skip(config, redis))]
pub async fn upload_files(
    claim: Claim,
    files: &[Utf8PathBuf],
    upload_path: &Utf8Path,
    config: Arc<Config>,
    redis: Arc<Redis>,
) -> Result<()> {
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
        let full_path = upload_path.join(file.file_name().unwrap_or("?"));
        let folder_id = get_or_create_folder_id(&hub, upload_path).await?;
        let full_path = full_path.as_str();
        info!(full_path, "Uploading file");
        let google_file = google_drive3::api::File {
            name: file.file_name().map(String::from),
            parents: Some(vec![folder_id]),
            ..Default::default()
        };
        let f = fs::File::open(file).await?;
        let c = hub.files().create(google_file);
        let span = info_span!("upload_file");
        span.record("filename", full_path);
        let _ = c
            .upload(f.into_std().await, guess_mime_from_file(file))
            .instrument(span)
            .await
            .wrap_err("failed to upload file")?;
    }
    Ok(())
}

#[instrument(skip_all)]
pub async fn get_or_create_folder_id(
    hub: &DriveHub<HttpsConnector<HttpConnector>>,
    path: &Utf8Path,
) -> Result<String> {
    let mut parent_id: Option<String> = Some("root".to_string());
    for part in path.iter().filter(|p| *p != "/") {
        let span = info_span!("get_folder_id");
        span.record("folder_name", part);
        let parent_id_query = parent_id
            .as_ref()
            .map(|id| format!(" and '{id}' in parents"))
            .unwrap_or_default();
        let query = format!("name='{part}' and mimeType='{FOLDER_MIME_TYPE}' {parent_id_query}");
        let (_, file_list) = hub
            .files()
            .list()
            .q(&query)
            .doit()
            .instrument(span)
            .await
            .wrap_err("failed to get folder id")?;

        let folder = if let Some(folder) = file_list.files.and_then(|f| f.into_iter().next()) {
            folder
        } else {
            create_folder(hub, part, parent_id.as_deref()).await?
        };
        parent_id = folder.id;
    }
    match parent_id {
        None => Err(eyre!("failed to get folder id for {:?}", path)),
        Some(parent_id) => Ok(parent_id),
    }
}

#[instrument(skip(hub))]
async fn create_folder(
    hub: &DriveHub<HttpsConnector<HttpConnector>>,
    folder_name: &str,
    parent_id: Option<&str>,
) -> Result<google_drive3::api::File> {
    let google_file = google_drive3::api::File {
        name: Some(folder_name.to_string()),
        parents: parent_id.map(|parent_id| vec![parent_id.to_string()]),
        mime_type: Some(FOLDER_MIME_TYPE.to_string()),
        ..Default::default()
    };
    let f = fs::File::open("/dev/null").await?;
    let span = info_span!("create_folder");
    span.record("folder_name", folder_name);
    let (_, folder) = hub
        .files()
        .create(google_file.clone())
        .upload(f.into_std().await, FOLDER_MIME_TYPE.parse()?)
        .instrument(span)
        .await
        .wrap_err("failed to create folder")?;
    Ok(folder)
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
