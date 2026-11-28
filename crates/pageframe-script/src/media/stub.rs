use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use url::Url;

use super::shared::{Command, Failure, MEDIA_ERR_SRC_NOT_SUPPORTED, Shared};
use crate::http::HttpClient;

pub(crate) const SUPPORTED: bool = false;

pub(crate) fn spawn(
    _: Url,
    http: HttpClient,
    commands: Receiver<Command>,
    shared: Arc<Shared>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("pageframe-media".to_string())
        .spawn(move || {
            drop((http, commands));
            shared.fail(Failure::new(
                MEDIA_ERR_SRC_NOT_SUPPORTED,
                "media playback is not supported on this platform",
            ));
        })
}
