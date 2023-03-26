use thiserror::Error;
use warp::reject::Reject;

#[derive(Debug, Error)]
pub enum Error {
    #[error("access denied")]
    AccessDenied,
    #[error("failed to process the input file")]
    Orc(#[source] color_eyre::Report),
    #[error("failed to cleanup")]
    Cleanup(#[source] color_eyre::Report),
    #[error("failed to upload")]
    Upload(#[source] color_eyre::Report),
    #[error("failed to save into the queue")]
    Queue(#[source] color_eyre::Report),
}

impl Reject for Error {}
