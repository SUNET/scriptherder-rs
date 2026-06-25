use thiserror::Error;

/// Errors raised while loading job or check files. Mirrors the Python
/// ScriptHerderError hierarchy (reason + filename).
#[derive(Debug, Error)]
pub enum ScriptHerderError {
    #[error("{reason} (file {filename})")]
    JobLoad { reason: String, filename: String },
    #[error("{reason} (file {filename})")]
    CheckLoad { reason: String, filename: String },
}

impl ScriptHerderError {
    pub fn job_load(reason: impl Into<String>, filename: impl Into<String>) -> Self {
        ScriptHerderError::JobLoad {
            reason: reason.into(),
            filename: filename.into(),
        }
    }
    pub fn check_load(reason: impl Into<String>, filename: impl Into<String>) -> Self {
        ScriptHerderError::CheckLoad {
            reason: reason.into(),
            filename: filename.into(),
        }
    }
    pub fn filename(&self) -> &str {
        match self {
            ScriptHerderError::JobLoad { filename, .. } => filename,
            ScriptHerderError::CheckLoad { filename, .. } => filename,
        }
    }
    pub fn reason(&self) -> &str {
        match self {
            ScriptHerderError::JobLoad { reason, .. } => reason,
            ScriptHerderError::CheckLoad { reason, .. } => reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_reason_and_filename() {
        let e = ScriptHerderError::check_load("Failed reading file", "/x.ini");
        assert_eq!(e.reason(), "Failed reading file");
        assert_eq!(e.filename(), "/x.ini");
    }
}
