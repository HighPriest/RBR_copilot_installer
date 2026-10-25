//! Who may see a package: bearer token → sha256 → credential row → decision.
//! Raw tokens are never stored or logged; only `hash_token` of one leaves a request.

mod access_decision;
mod bearer_header;
mod token_lookup;
mod token_secret;

pub use access_decision::{decide, now, Decision, TokenRow};
pub use bearer_header::bearer_from_headers;
pub use token_lookup::load_token;
pub use token_secret::{hash_token, mint_token, well_formed, TOKEN_PREFIX};
