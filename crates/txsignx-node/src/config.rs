use crate::NodeError;
use std::{fs::File, io::Read, path::Path};
/// Exact grammar, not general URL normalization. No DNS, paths, redirects or credentials.
pub struct RpcEndpoint(String);
impl RpcEndpoint {
    pub fn parse(url: &str) -> Result<Self, NodeError> {
        let port = url
            .strip_prefix("http://127.0.0.1:")
            .or_else(|| url.strip_prefix("http://[::1]:"))
            .ok_or(NodeError::InvalidEndpoint)?;
        if port.is_empty()
            || port.starts_with('0')
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().ok().filter(|n| *n > 0).is_none()
        {
            return Err(NodeError::InvalidEndpoint);
        }
        Ok(Self(url.to_owned()))
    }
    pub(crate) fn url(&self) -> &str {
        &self.0
    }
}
pub(crate) fn cookie(path: &Path) -> Result<String, NodeError> {
    // Reject special files before open so a FIFO cannot block authentication setup.
    if !std::fs::metadata(path)
        .map_err(|_| NodeError::InvalidCookie)?
        .is_file()
    {
        return Err(NodeError::InvalidCookie);
    }
    let file = File::open(path).map_err(|_| NodeError::InvalidCookie)?;
    if !file
        .metadata()
        .map_err(|_| NodeError::InvalidCookie)?
        .is_file()
    {
        return Err(NodeError::InvalidCookie);
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| NodeError::InvalidCookie)?;
    if bytes.len() > 4096 {
        return Err(NodeError::InvalidCookie);
    }
    let text = String::from_utf8(bytes).map_err(|_| NodeError::InvalidCookie)?;
    let text = text.trim_end_matches(['\r', '\n']);
    let (user, secret) = text.split_once(':').ok_or(NodeError::InvalidCookie)?;
    if user != "__cookie__"
        || secret.is_empty()
        || !secret.bytes().all(|b| b.is_ascii_graphic() && b != b':')
    {
        return Err(NodeError::InvalidCookie);
    }
    Ok(text.to_owned())
}
