//! First authentication and later linking use fresh public ActivityPub posts.
//! Database challenge consumption and session issuance belong to member service.
pub use super::federation::{FederationClient, FederationError, ResolvedActor, VerifiedIdentity};

pub fn client_with_signer(
    signer: std::sync::Arc<dyn super::federation::http_signature::RequestSigner>,
) -> FederationClient<super::federation::transport::HttpTransport> {
    FederationClient::new_shared(super::federation::transport::HttpTransport::new(signer))
}
