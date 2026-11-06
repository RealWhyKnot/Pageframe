use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} can't be turned into a file URL", path.display())]
    FileUrl { path: PathBuf },
    #[error("invalid selector `{selector}`")]
    Selector { selector: String },
    #[error("run the window event loop")]
    EventLoop(#[from] winit::error::EventLoopError),
}
