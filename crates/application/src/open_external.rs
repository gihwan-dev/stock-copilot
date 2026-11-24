use std::future::Future;

use domain::{
    AuthorizedExternalUrl, ExternalUrlAllowance, ExternalUrlInput, ExternalUrlRejection,
    NormalizedUrl, ParsedExternalUrl,
};

use crate::{
    BugDiagnostic, ClassifiedError, ErrorClass, RequestRejection, RetryDelay, SystemDiagnostic,
    SystemPortError, UserAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalUrlOpened;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUrlOpenerError {
    PermissionDenied,
    Unavailable(RetryDelay),
    LaunchFailed,
}

impl ClassifiedError for ExternalUrlOpenerError {
    fn class(&self) -> ErrorClass {
        match *self {
            Self::PermissionDenied => ErrorClass::UserAction(UserAction::CheckPermissions),
            Self::Unavailable(retry) => ErrorClass::Wait(retry),
            Self::LaunchFailed => ErrorClass::Wait(RetryDelay::Unknown),
        }
    }
}

pub trait CollectedUrlCatalog: Send + Sync {
    fn find_collected(
        &self,
        url: &ParsedExternalUrl,
    ) -> impl Future<Output = Result<Option<NormalizedUrl>, SystemPortError>> + Send;
}

pub trait ExternalUrlOpener: Send + Sync {
    fn open(
        &self,
        url: &AuthorizedExternalUrl,
    ) -> impl Future<Output = Result<ExternalUrlOpened, ExternalUrlOpenerError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenExternalError {
    Rejected(ExternalUrlRejection),
    NotAllowed,
    CatalogMismatch,
    Catalog(SystemPortError),
    Opener(ExternalUrlOpenerError),
}

impl ClassifiedError for OpenExternalError {
    fn class(&self) -> ErrorClass {
        match *self {
            Self::Rejected(_) | Self::NotAllowed => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
            }
            Self::CatalogMismatch => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::RepositoryIntegrity))
            }
            Self::Catalog(error) => error.class(),
            Self::Opener(error) => error.class(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenExternalOutcome {
    allowance: ExternalUrlAllowance,
    opened: ExternalUrlOpened,
}

impl OpenExternalOutcome {
    #[must_use]
    pub const fn new(allowance: ExternalUrlAllowance, opened: ExternalUrlOpened) -> Self {
        Self { allowance, opened }
    }

    #[must_use]
    pub const fn allowance(self) -> ExternalUrlAllowance {
        self.allowance
    }

    #[must_use]
    pub const fn opened(self) -> ExternalUrlOpened {
        self.opened
    }
}

#[derive(Debug)]
pub struct OpenExternalUrl<'ports, C, O> {
    catalog: &'ports C,
    opener: &'ports O,
}

impl<'ports, C, O> OpenExternalUrl<'ports, C, O> {
    #[must_use]
    pub const fn new(catalog: &'ports C, opener: &'ports O) -> Self {
        Self { catalog, opener }
    }
}

impl<C, O> OpenExternalUrl<'_, C, O>
where
    C: CollectedUrlCatalog,
    O: ExternalUrlOpener,
{
    pub async fn execute(
        &self,
        input: &ExternalUrlInput,
    ) -> Result<OpenExternalOutcome, OpenExternalError> {
        let parsed = ParsedExternalUrl::parse(input).map_err(OpenExternalError::Rejected)?;
        let authorized = if let Some(fixed) = AuthorizedExternalUrl::authorize_fixed(parsed.clone())
        {
            fixed
        } else {
            let collected = self
                .catalog
                .find_collected(&parsed)
                .await
                .map_err(OpenExternalError::Catalog)?
                .ok_or(OpenExternalError::NotAllowed)?;
            AuthorizedExternalUrl::authorize_collected(parsed, &collected)
                .map_err(|_rejection| OpenExternalError::CatalogMismatch)?
        };
        let opened = self
            .opener
            .open(&authorized)
            .await
            .map_err(OpenExternalError::Opener)?;
        Ok(OpenExternalOutcome::new(authorized.allowance(), opened))
    }
}
