use std::marker::PhantomData;

use hyper::header;
use rustc_serialize::json::Json;
use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::{ser, de};

use super::{Token, Lifetime};
use client::response::{FromResponse, ParseError, JsonHelper};

/// The bearer token type.
///
/// See [RFC 6750](http://tools.ietf.org/html/rfc6750).
#[derive(Debug, Clone, PartialEq, Eq, RustcEncodable, RustcDecodable)]
pub struct Bearer<L: Lifetime> {
    access_token: String,
    scope: Option<String>,
    lifetime: L,
    id_token: Option<String>,
}

impl<L: Lifetime> Token<L> for Bearer<L> {
    fn access_token(&self) -> &str { &self.access_token }
    fn scope(&self) -> Option<&str> { self.scope.as_ref().map(|s| &s[..]) }
    fn lifetime(&self) -> &L { &self.lifetime }
    fn id_token(&self) -> Option<&str> { self.id_token.as_ref().map(|s| &s[..]) }
}

impl<'a, L: Lifetime> Into<header::Authorization<header::Bearer>> for &'a Bearer<L> {
    fn into(self) -> header::Authorization<header::Bearer> {
        header::Authorization(header::Bearer { token: self.access_token.clone() })
    }
}

impl<L: Lifetime> Bearer<L> {
    fn from_response_and_lifetime(json: &Json, lifetime: L) -> Result<Self, ParseError> {
        let obj = try!(JsonHelper(json).as_object());

        let token_type = try!(obj.get_string("token_type"));
        if token_type != "Bearer" && token_type != "bearer" {
            return Err(ParseError::ExpectedFieldValue("token_type", "Bearer"));
        }

        let access_token = try!(obj.get_string("access_token"));
        let scope = obj.get_string_option("scope");
        let id_token = obj.get_string_option("id_token");

        Ok(Bearer {
            access_token: access_token.into(),
            scope: scope.map(Into::into),
            lifetime: lifetime,
            id_token: id_token.map(Into::into),
        })
    }
}

impl<L: Lifetime> FromResponse for Bearer<L> {
    fn from_response(json: &Json) -> Result<Self, ParseError> {
        let lifetime = try!(FromResponse::from_response(json));
        Bearer::from_response_and_lifetime(json, lifetime)
    }

    fn from_response_inherit(json: &Json, prev: &Self) -> Result<Self, ParseError> {
        let lifetime = try!(FromResponse::from_response_inherit(json, &prev.lifetime));
        Bearer::from_response_and_lifetime(json, lifetime)
    }
}

impl<L: Lifetime + Serialize> Serialize for Bearer<L> {
    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
        serializer.visit_struct("Bearer", SerVisitor(self, 0))
    }
}

struct SerVisitor<'a, L: Lifetime + Serialize + 'a>(&'a Bearer<L>, u8);
impl<'a, L: Lifetime + Serialize + 'a> ser::MapVisitor for SerVisitor<'a, L> {
    fn visit<S: Serializer>(&mut self, serializer: &mut S) -> Result<Option<()>, S::Error> {
        self.1 += 1;
        match self.1 {
            1 => serializer.visit_struct_elt("access_token", &self.0.access_token).map(Some),
            2 => serializer.visit_struct_elt("scope", &self.0.scope).map(Some),
            3 => serializer.visit_struct_elt("lifetime", &self.0.lifetime).map(Some),
            4 => serializer.visit_struct_elt("id_token", &self.0.id_token).map(Some),
            _ => Ok(None),
        }
    }

    fn len(&self) -> Option<usize> { Some(4) }
}

impl<L: Lifetime + Deserialize> Deserialize for Bearer<L> {
    fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
        static FIELDS: &'static [&'static str] = &["access_token", "scope", "lifetime", "id_token"];
        deserializer.visit_struct("Bearer", FIELDS, DeVisitor(PhantomData))
    }
}

struct DeVisitor<L: Lifetime + Deserialize>(PhantomData<L>);
impl<L: Lifetime + Deserialize> de::Visitor for DeVisitor<L> {
    type Value = Bearer<L>;

    fn visit_map<V: de::MapVisitor>(&mut self, mut visitor: V) -> Result<Bearer<L>, V::Error> {
        let mut access_token = None;
        let mut scope = None;
        let mut lifetime = None;
        let mut id_token = None;

        loop {
            match try!(visitor.visit_key()) {
                Some(Field::AccessToken) => access_token = Some(try!(visitor.visit_value())),
                Some(Field::Scope) => scope = Some(try!(visitor.visit_value())),
                Some(Field::Lifetime) => lifetime = Some(try!(visitor.visit_value())),
                Some(Field::IdToken) => id_token = Some(try!(visitor.visit_value())),
                None => break,
            }
        }

        let access_token = match access_token {
            Some(s) => s,
            None => return visitor.missing_field("access_token"),
        };
        let lifetime = match lifetime {
            Some(l) => l,
            None => return visitor.missing_field("lifetime"),
        };

        try!(visitor.end());

        Ok(Bearer {
            access_token: access_token,
            scope: scope,
            lifetime: lifetime,
            id_token: id_token,
        })
    }
}

enum Field {
    AccessToken,
    Scope,
    Lifetime,
    IdToken,
}

impl Deserialize for Field {
    fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
        deserializer.visit(FieldVisitor)
    }
}

struct FieldVisitor;
impl de::Visitor for FieldVisitor {
    type Value = Field;

    fn visit_str<E: de::Error>(&mut self, value: &str) -> Result<Field, E> {
        match value {
            "access_token" => Ok(Field::AccessToken),
            "scope" => Ok(Field::Scope),
            "lifetime" => Ok(Field::Lifetime),
            "id_token" => Ok(Field::IdToken),
            _ => Err(de::Error::syntax("expected access_token, scope, lifetime or id_token")),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{UTC, Duration};
    use rustc_serialize::json::Json;
    use serde_json;

    use client::response::{FromResponse, ParseError};
    use token::{Static, Expiring};
    use super::Bearer;

    #[test]
    fn from_response_with_invalid_token_type() {
        let json = Json::from_str(r#"{"token_type":"MAC","access_token":"aaaaaaaa"}"#).unwrap();
        assert_eq!(
            ParseError::ExpectedFieldValue("token_type", "Bearer"),
            Bearer::<Static>::from_response(&json).unwrap_err()
        );
    }

    #[test]
    fn from_response_capital_b() {
        let json = Json::from_str(r#"{"token_type":"Bearer","access_token":"aaaaaaaa"}"#).unwrap();
        assert_eq!(
            Bearer {
                access_token: String::from("aaaaaaaa"),
                scope: None,
                lifetime: Static,
                id_token: None,
            },
            Bearer::<Static>::from_response(&json).unwrap()
        );
    }

    #[test]
    fn from_response_little_b() {
        let json = Json::from_str(r#"{"token_type":"bearer","access_token":"aaaaaaaa"}"#).unwrap();
        assert_eq!(
            Bearer {
                access_token: String::from("aaaaaaaa"),
                scope: None,
                lifetime: Static,
                id_token: None,
            },
            Bearer::<Static>::from_response(&json).unwrap()
        );
    }

    #[test]
    fn from_response_with_scope() {
        let json = Json::from_str(
            r#"{"token_type":"Bearer","access_token":"aaaaaaaa","scope":"foo"}"#
        ).unwrap();
        assert_eq!(
            Bearer {
                access_token: String::from("aaaaaaaa"),
                scope: Some(String::from("foo")),
                lifetime: Static,
                id_token: None,
            },
            Bearer::<Static>::from_response(&json).unwrap()
        );
    }

    #[test]
    fn from_response_expiring() {
        let json = Json::from_str(r#"
            {
                "token_type":"Bearer",
                "access_token":"aaaaaaaa",
                "expires_in":3600,
                "refresh_token":"bbbbbbbb"
            }
        "#).unwrap();
        let bearer = Bearer::<Expiring>::from_response(&json).unwrap();
        assert_eq!("aaaaaaaa", bearer.access_token);
        assert_eq!(None, bearer.scope);
        let expiring = bearer.lifetime;
        assert_eq!("bbbbbbbb", expiring.refresh_token());
        assert!(expiring.expires() > &UTC::now());
        assert!(expiring.expires() <= &(UTC::now() + Duration::seconds(3600)));
    }

    #[test]
    fn from_response_inherit_expiring() {
        let json = Json::from_str(r#"
            {
                "token_type":"Bearer",
                "access_token":"aaaaaaaa",
                "expires_in":3600,
                "refresh_token":"bbbbbbbb"
            }
        "#).unwrap();
        let prev = Bearer::<Expiring>::from_response(&json).unwrap();

        let json = Json::from_str(r#"
            {
                "token_type":"Bearer",
                "access_token":"cccccccc",
                "expires_in":3600
            }
        "#).unwrap();
        let bearer = Bearer::<Expiring>::from_response_inherit(&json, &prev).unwrap();
        assert_eq!("cccccccc", bearer.access_token);
        assert_eq!(None, bearer.scope);
        let expiring = bearer.lifetime;
        assert_eq!("bbbbbbbb", expiring.refresh_token());
        assert!(expiring.expires() > &UTC::now());
        assert!(expiring.expires() <= &(UTC::now() + Duration::seconds(3600)));
    }

    #[test]
    fn serialize_deserialize() {
        let original = Bearer {
            access_token: String::from("foo"),
            scope: Some(String::from("bar")),
            lifetime: Static,
            id_token: Some(String::from("baz")),
        };
        let serialized = serde_json::to_value(&original);
        let deserialized = serde_json::from_value(serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
