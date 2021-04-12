use openlimits::errors::OpenLimitsError;
use crate::exchanges::binance::FilterError;

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
