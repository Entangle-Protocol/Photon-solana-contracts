use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtensionError {
    #[error("Extension error")]
    Extension,
    #[error("Signing error")]
    Sign,
}
