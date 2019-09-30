use std::{fmt, marker::PhantomData};
use bytes::{Bytes, BytesMut};
use serde::{ser::Serialize, de::DeserializeOwned};
use crate::Format;

#[derive(Copy, Clone, Debug)]
pub enum Cookie {
	Oneshot,
	Single(u16),
	Stream(u16),
}

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

	pub fn oneshot(length: Option<usize>) -> Self {
		Self {
			cookie: 0,
			length: Self::make_length(length),
		}
	}

	pub fn single(cookie: u16, length: Option<usize>) -> Self {
		Self {
			cookie: cookie,
			length: Self::make_length(length),
		}
	}

	pub fn stream(cookie: u16, length: Option<usize>) -> Self {
		Self {
			cookie: cookie | 0x8000,
			length: Self::make_length(length),
		}
	}

	pub fn cookie(&self) -> Option<u16> {
		match self.cookie & !0x8000 {
			0 => None,
			v => Some(v)
		}
	}

	pub fn length(&self) -> usize {
		0xfffe.min(self.length).into()
	}

	pub fn has_more_packets(&self) -> bool {
		(self.cookie & 0x8000) != 0
	}

	pub fn has_more_payload(&self) -> bool {
		self.length == 0xffff
	}
}
#[derive(Clone)]
pub struct Packet<F = ()> {
	cookie: Cookie,
	bytes: Bytes,

	_marker: PhantomData<F>,
}

impl<F> fmt::Debug for Packet<F> {
	fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		write!(f, "Packet {{ cookie: {:?}, bytes: {:?} }}", self.cookie, self.bytes)
	}
}

impl<F: Format> Packet<F> {
	pub fn new(cookie: Cookie, bytes: Bytes) -> Self {
		Self { cookie, bytes, _marker: PhantomData }
	}

	pub fn oneshot<T: Serialize>(value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Oneshot,
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	pub fn single<T: Serialize>(cookie: u16, value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Single(cookie),
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	pub fn stream<T: Serialize>(cookie: u16, value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			cookie: Cookie::Stream(cookie),
			bytes:  bytes.freeze(),

			_marker: PhantomData,
		})
	}

	pub fn cookie(&self) -> Cookie {
		self.cookie
	}

	pub fn bytes(&self) -> &Bytes {
		&self.bytes
	}

	pub fn cast<T: DeserializeOwned>(&self) -> Result<T, F::DeserializeError> {
		F::deserialize(&self.bytes)
	}
}


