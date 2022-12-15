//! Helper

#[cfg(feature = "serde")]
pub mod serde {
    //! Helper modules to implement [`serde::Deserialize`] and [`serde::Serialize`]
    pub mod u8_array {
        //! serialize/deserializer module for [u8;N]
        //! ```rust
        //! use serde::{Serialize, Deserialize};
        //! #[derive(Serialize, Deserialize)]
        //! struct Foo {
        //!     #[serde(with = "movparse_box::util::serde::u8_array")]
        //!     hoge: [u8;3],
        //!     #[serde(with = "movparse_box::util::serde::u8_array")]
        //!     bar: [u8;124],
        //! }
        //! ```
        use serde::{de::Visitor, ser::SerializeTuple, Deserializer, Serializer};

        pub fn serialize<S, const N: usize>(input: &[u8; N], ser: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut tuple = ser.serialize_tuple(N)?;
            for elem in input {
                tuple.serialize_element(elem)?;
            }
            tuple.end()
        }

        struct U8ArrayVisitor<const N: usize>;

        impl<'de, const N: usize> Visitor<'de> for U8ArrayVisitor<N> {
            type Value = [u8; N];

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_fmt(format_args!("[u8; {}] array expected", N))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut value = [0u8; N];
                for elem in value.iter_mut() {
                    *elem = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(N, &self))?;
                }
                Ok(value)
            }
        }

        pub fn deserialize<'de, D, const N: usize>(de: D) -> Result<[u8; N], D::Error>
        where
            D: Deserializer<'de>,
        {
            de.deserialize_tuple(N, U8ArrayVisitor)
        }
    }
}
