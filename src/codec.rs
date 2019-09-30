use std::{io, usize, pin::Pin, marker::PhantomData};
use tokio::{self, codec::{Decoder, Encoder}};
use bytes::{BufMut, Bytes, BytesMut, ByteOrder, BigEndian};
use futures::{stream::{Stream, StreamExt}, sink::{Sink, SinkExt}};
use crate::{Format, reframe::{self, Reframe}, Header, Cookie, Packet};

/// `tokio::{Decoder, Encoder}` to transform a `Stream + Sink` of bytes to one
/// of header and payload.
pub struct Codec;

impl Default for Codec {
	fn default() -> Self {
		Self
	}
}

impl Decoder for Codec {
	type Item = (Header, Bytes);
	type Error = io::Error;

	fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<(Header, Bytes)>, io::Error> {
		if buf.len() < 4 {
			return Ok(None);
		}

		let header = Header {
			cookie: BigEndian::read_u16(&buf[0..]),
			length: BigEndian::read_u16(&buf[2..])
		};

		if buf.len() - 4 < usize::from(header.length()) {
			return Ok(None);
		}

		let payload = buf.split_off(4);

		Ok(Some((header, payload.freeze())))
	}
}

impl Encoder for Codec {
	type Item = (Header, Bytes);
	type Error = io::Error;

	fn encode(&mut self, (header, payload): (Header, Bytes), buf: &mut BytesMut) -> Result<(), io::Error> {
		buf.reserve(4 + payload.len());

		buf.put_u16_be(header.cookie);
		buf.put_u16_be(header.length);
		buf.put_slice(&payload);

		Ok(())
	}
}

/// Reframe a `Codec` into a `Packet`.
#[derive(Copy, Clone, Debug)]
pub struct Packets<F = ()> {
	_marker: PhantomData<F>
}

impl<F: Format + ::std::fmt::Debug> Reframe for Packets<F> {
	type Input = (Header, Bytes);
	type Output = Packet<F>;
	type Error = io::Error;

  fn stream(mut stream: Pin<Box<dyn Stream<Item = io::Result<Self::Input>> + Send>>) -> Pin<Box<dyn Stream<Item = io::Result<Self::Output>> + Send>> {
		Box::pin(reframe::stream::<Self, _, _>(|mut tx| async move {
			macro_rules! next {
				($body:expr) => (
					if let Some(value) = stream.next().await {
						match value {
							Ok(value) => value,

							Err(error) => {
								tx.send(Err(error)).await.unwrap();
								return;
							}
						}
					}
					else {
						return;
					}
				);
			}

			loop {
				let mut payload = BytesMut::new();

				let mut packet = next!(stream);
				payload.extend_from_slice(&packet.1);

				while packet.0.has_more_payload() {
					packet = next!(stream);
					payload.extend_from_slice(&packet.1);
				}

				tx.send(Ok(if let Some(cookie) = packet.0.cookie() {
					if packet.0.has_more_packets() {
						Packet::<F>::new(Cookie::Stream(cookie), payload.freeze())
					}
					else {
						Packet::<F>::new(Cookie::Single(cookie), payload.freeze())
					}
				}
				else {
					Packet::<F>::new(Cookie::Oneshot, payload.freeze())
				})).await.unwrap();
			}
		}))
	}

  fn sink(mut sink: Pin<Box<dyn Sink<Self::Input, Error = Self::Error> + Send>>) -> Pin<Box<dyn Sink<Self::Output, Error = Self::Error> + Send>> {
		Box::pin(reframe::sink::<Self, _, _>(|mut rx| async move {
			while let Some(packet) = rx.next().await : Option<Packet<F>> {
				let mut chunks = (0 ..= packet.bytes().len() / 0xfffe).peekable();

				while let Some(chunk) = chunks.next() {
					let is_last = chunks.peek().is_none();
					let payload = packet.bytes().slice(chunk * 0xfffe, packet.bytes().len() - (chunk * 0xfffe));
					let length  = if is_last { Some(payload.len()) } else { None };
					let header  = match packet.cookie() {
						Cookie::Oneshot =>
							Header::oneshot(length),

						Cookie::Single(cookie) =>
							Header::single(cookie, length),

						Cookie::Stream(cookie) =>
							Header::stream(cookie, length),
					};

					sink.send((header, payload)).await.unwrap();
				}
			}
		}).sink_map_err(|err| io::Error::new(io::ErrorKind::Interrupted, err)))
	}
}
