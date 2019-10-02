use std::{pin::Pin, marker::PhantomData};
use futures::{stream::{Stream, StreamExt}, sink::{Sink, SinkExt}, task::{Context, Poll}};
use tokio::{stream, future, sync::mpsc::{UnboundedSender, error::UnboundedSendError, unbounded_channel}};
use crate::{Format, packet::{self, Packet}, message::{self, Message}};

/// A full message session (i.e. bound to a cookie).
pub struct Session<F = ()> {
	sender: UnboundedSender<Packet<F>>,
	stream: Pin<Box<dyn Stream<Item = Message<F>> + Send>>,
	sink: Pin<Box<dyn Sink<Message<F>, Error = UnboundedSendError> + Send>>,
}

/// A sink that takes no replies.
pub struct NoReply<I, E> {
	_marker: PhantomData<(I, E)>,
}

impl<I, E> Default for NoReply<I, E> {
	fn default() -> Self {
		Self {
			_marker: PhantomData,
		}
	}
}

impl<I, E> Sink<I> for NoReply<I, E> {
	type Error = E;

	fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Pending
	}

	fn start_send(self: Pin<&mut Self>, _item: I) -> Result<(), Self::Error> {
		Ok(())
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Pending
	}

	fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Pending
	}
}

impl<F: Format> Session<F> {
	pub fn no_reply<M: Into<Message<F>>>(value: M) -> Self {
		Self {
			sender: unbounded_channel().0,
			stream: Box::pin(stream::once(future::ready(value.into()))),
			sink: Box::pin(NoReply::<Message<F>, _>::default()),
		}
	}

	pub fn new(cookie: u16, mut sink: impl Sink<Packet<F>> + Send + Unpin + 'static) -> Self {
		let (packet_tx, packet_rx) = unbounded_channel::<Packet<F>>();
		let (input_tx, mut input_rx) = unbounded_channel::<Message<F>>();

		tokio::spawn(async move {
			while let Some(message) = input_rx.next().await : Option<Message<F>> {
				sink.send(Packet::new(match message.mode {
					message::Mode::NoReply =>
						packet::Cookie::Oneshot,

					message::Mode::More =>
						packet::Cookie::Stream(cookie),

					message::Mode::End =>
						packet::Cookie::Single(cookie),
				}, message.bytes)).await.ok();
			}
		});

		Self {
			sender: packet_tx,
			stream: Box::pin(packet_rx.map(|p| Message::<F>::from(p))),
			sink: Box::pin(input_tx),
		}
	}

	pub fn sender(&self) -> UnboundedSender<Packet<F>> {
		self.sender.clone()
	}
}

impl<F: Format> Stream for Session<F> {
	type Item = Message<F>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		Pin::new(&mut self.get_mut().stream).poll_next(cx)
	}
}

impl<F: Format> Sink<Message<F>> for Session<F> {
	type Error = UnboundedSendError;

	fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_ready(cx)
	}

	fn start_send(self: Pin<&mut Self>, item: Message<F>) -> Result<(), Self::Error> {
		Pin::new(&mut Pin::get_mut(self).sink).start_send(item)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_flush(cx)
	}

	fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_close(cx)
	}
}
