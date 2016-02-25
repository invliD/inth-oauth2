//! Tokens.
//!
//! Access token types are abstracted through the `Token` trait. See
//! [RFC 6749, section 7.1](http://tools.ietf.org/html/rfc6749#section-7.1).
//!
//! Expiring and non-expiring tokens are abstracted through the `Lifetime` trait.

use client::response::FromResponse;

/// OAuth 2.0 tokens.
///
/// See [RFC 6749, section 5](http://tools.ietf.org/html/rfc6749#section-5).
pub trait Token<L: Lifetime>: FromResponse {
    /// Returns the access token.
    ///
    /// See [RF C6749, section 1.4](http://tools.ietf.org/html/rfc6749#section-1.4).
    fn access_token(&self) -> &str;

    /// Returns the scope, if available.
    fn scope(&self) -> Option<&str>;

    /// Returns the token lifetime.
    fn lifetime(&self) -> &L;

    /// Returns the ID token (if this is an OpenID Connect token).
    fn id_token(&self) -> Option<&str>;
}

/// OAuth 2.0 token lifetimes.
pub trait Lifetime: FromResponse {
    /// Returns true if the access token is no longer valid.
    fn expired(&self) -> bool;
}

pub use self::bearer::Bearer;
mod bearer;

pub use self::statik::Static;
mod statik;

pub use self::expiring::Expiring;
mod expiring;
