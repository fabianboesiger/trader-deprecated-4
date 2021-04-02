use openlimits::errors::OpenLimitsError;

#[derive(Debug)]
pub enum Error {
    OpenLimits(OpenLimitsError),
    Filter,
}

impl From<OpenLimitsError> for Error {
    fn from(err: OpenLimitsError) -> Self {
        Self::OpenLimits(err)
    }
}
