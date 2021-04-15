use crate::exchanges::binance::FilterError;
use openlimits::errors::OpenLimitsError;

#[derive(Debug)]
pub enum Error {
    OpenLimits(OpenLimitsError),
    Filter(FilterError),
}

impl From<OpenLimitsError> for Error {
    fn from(err: OpenLimitsError) -> Self {
        Self::OpenLimits(err)
    }
}
