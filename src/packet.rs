use std::{fmt, marker::PhantomData};
use bytes::{Bytes, BytesMut};
use serde::{ser::Serialize, de::DeserializeOwned};
use crate::Format;

/// The cookie for a packet.
#[derive(Copy, Clone, Debug)]
pub enum Cookie {
	/// There is no cookie, this packet will not receive any replies.
	Oneshot,

	/// There are no more packets in this stream.
	Single(u16),

	/// There are more packets in this stream.
	Stream(u16),
}

/// Header for a `protociolla::Packet`.
#[derive(Copy, Clone, Debug)]
pub struct Header {
	pub(crate) cookie: u16,
	pub(crate) length: u16,
}

impl Header {
	fn make_length(value: Option<usize>) -> u16 {
		if let Some(value) = value {
			value as u16
		}
		else {
			0xffff
		}
	}

	/// Construct a `Header` with a `Oneshot` cookie.
	pub fn oneshot(length: Option<usize>) -> Self {
		Self {
			cookie: 0,
			length: Self::make_length(length),
		}
	}

	/// Construct a `Header` with a `Single` cookie.
	pub fn single(cookie: u16, length: Option<usize>) -> Self {
		Self {
			cookie: cookie,
			length: Self::make_length(length),
		}
	}

	/// Construct a `Header` with a `Stream` cookie.
	pub fn stream(cookie: u16, length: Option<usize>) -> Self {
		Self {
			cookie: cookie | 0x8000,
			length: Self::make_length(length),
		}
	}

	/// Get the cookie, if any.
	pub fn cookie(&self) -> Option<u16> {
		match self.cookie & !0x8000 {
			0 => None,
			v => Some(v)
		}
	}

	/// Get the length of the payload.
	pub fn length(&self) -> usize {
		0xfffe.min(self.length).into()
	}

	/// Check if there are more packets in this stream.
	pub fn has_more_packets(&self) -> bool {
		(self.cookie & 0x8000) != 0
	}

	/// Check if there are more data fragments in this packet.
	pub fn has_more_payload(&self) -> bool {
		self.length == 0xffff
	}
}

/// A fully formed packet (with defragmented payload).
#[derive(Clone)]
pub struct Packet<F = ()> {
	pub(crate) cookie: Cookie,
	pub(crate) bytes: Bytes,

	_marker: PhantomData<F>,
}

impl<F> fmt::Debug for Packet<F> {
	fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		write!(f, "Packet {{ cookie: {:?}, bytes: {:?} }}", self.cookie, self.bytes)
	}
}

impl Packet<()> {
	/// Decide on the `Format` to use for this packet.
	pub fn with_format<F: Format>(self) -> Self {
		Self {
			cookie: self.cookie,
			bytes: self.bytes,

			_marker: PhantomData,
		}
	}
}

impl<F: Format> Packet<F> {
	/// Create a packet from a `cookie` and payload.
	pub fn new(cookie: Cookie, payload: Bytes) -> Self {
		Self { cookie, bytes: payload, _marker: PhantomData }
	}

	/// Create a new oneshot packet from a value.
	pub fn oneshot<T: Serialize>(value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Oneshot,
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// Create a new single packet from a value.
	pub fn single<T: Serialize>(cookie: u16, value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Single(cookie),
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// Create a new stream packet from a value.
	pub fn stream<T: Serialize>(cookie: u16, value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Stream(cookie),
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// The cookie for the packet.
	pub fn cookie(&self) -> Cookie {
		self.cookie
	}

	/// The payload of the packet.
	pub fn bytes(&self) -> &Bytes {
		&self.bytes
	}

	/// Try to deserialize the payload to a value.
	pub fn cast<T: DeserializeOwned>(&self) -> Result<T, F::DeserializeError> {
		F::deserialize(&self.bytes)
	}
}
