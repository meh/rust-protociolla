use std::{fmt, marker::PhantomData};
use bytes::{Bytes, BytesMut};
use serde::{ser::Serialize, de::DeserializeOwned};
use crate::{packet::{self, Packet}, Format};

/// A message.
#[derive(Clone)]
pub struct Message<F = ()> {
	pub(crate) mode: Mode,
	pub(crate) bytes: Bytes,

	_marker: PhantomData<F>,
}

impl<F> fmt::Debug for Message<F> {
	fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		write!(f, "UnboundPacket {{ mode: {:?}, bytes: {:?} }}", self.mode, self.bytes)
	}
}

impl<F> From<Packet<F>> for Message<F> {
	fn from(packet: Packet<F>) -> Self {
		Message {
			mode: match packet.cookie {
				packet::Cookie::Oneshot =>
					Mode::NoReply,

				packet::Cookie::Single(..) =>
					Mode::End,

				packet::Cookie::Stream(..) =>
					Mode::More,
			},

			bytes: packet.bytes,
			_marker: PhantomData,
		}
	}
}

/// The message mode.
#[derive(Copy, Clone, Debug)]
pub enum Mode {
	/// The message cannot receive a reply.
	NoReply,

	/// There will be more messages after this.
	More,

	/// There will not be any more messages after this.
	End,
}

impl Message<()> {
	/// Decide on the `Format` to use for this packet.
	pub fn with_format<F: Format>(self) -> Self {
		Self {
			mode: self.mode,
			bytes: self.bytes,

			_marker: PhantomData,
		}
	}
}

impl<F: Format> Message<F> {
	/// Create a packet from a `cookie` and payload.
	pub fn new(mode: Mode, payload: Bytes) -> Self {
		Self { mode, bytes: payload, _marker: PhantomData }
	}

	/// Create a new oneshot packet from a value.
	pub fn no_reply<T: Serialize>(value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			mode:  Mode::NoReply,
			bytes: bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// Create a new message marking more messages incoming.
	pub fn more<T: Serialize>(value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			mode:  Mode::More,
			bytes: bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// Create a new ending message from a value.
	pub fn end<T: Serialize>(value: &T) -> Result<Self, F::SerializeError> {
		let mut bytes = BytesMut::new();
		F::serialize(value, &mut bytes)?;

		Ok(Self {
			mode:  Mode::End,
			bytes: bytes.freeze(),

			_marker: PhantomData,
		})
	}

	/// The message mode.
	pub fn mode(&self) -> Mode {
		self.mode
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
