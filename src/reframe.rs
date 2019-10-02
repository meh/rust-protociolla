use std::{future::Future, pin::Pin, task::{Context, Poll}};
use futures::{stream::{Stream, StreamExt}, sink::Sink};
use tokio::sync::mpsc::{self, channel, Receiver, Sender};

pub fn stream<Into, F, O>(func: F) -> Receiver<Into>
	where F: FnOnce(mpsc::Sender<Into>) -> O,
	      O: Future<Output = ()> + Send + 'static
{
	let (tx, rx) = channel(16);
	tokio::spawn(func(tx));
	rx
}

pub fn sink<Into, F, O>(func: F) -> Sender<Into>
	where F: FnOnce(mpsc::Receiver<Into>) -> O,
	      O: Future<Output = ()> + Send + 'static
{
	let (tx, rx) = channel(16);
	tokio::spawn(func(rx));
	tx
}

pub struct Source<St, Si, Err>
	where St: Send + 'static,
	      Si: Send + 'static,
	      Err: Send + 'static
{
	pub stream: Pin<Box<dyn Stream<Item = Result<St, Err>> + Send>>,
	pub sink: Pin<Box<dyn Sink<Si, Error = Err> + Send>>,
}

impl<St, Si, Err> Source<St, Si, Err>
	where St: Send + 'static,
	      Si: Send + 'static,
	      Err: Send + 'static
{
	pub fn new(stream: impl Stream<Item = Result<St, Err>> + Send + 'static, sink: impl Sink<Si, Error = Err> + Send + 'static) -> Self {
		Self {
			stream: Box::pin(stream),
			sink: Box::pin(sink),
		}
	}
}

pub trait Reframe {
	type StreamFrom: Send + 'static;
	type StreamInto: Send + 'static;

	type SinkFrom: Send + 'static;
	type SinkInto: Send + 'static;

	type Error: Send + 'static;

	fn reframe(source: Source<Self::StreamFrom, Self::SinkFrom, Self::Error>) ->
		Source<Self::StreamInto, Self::SinkInto, Self::Error>;
}

pub struct Reframed<R: Reframe> {
	stream: Pin<Box<dyn Stream<Item = Result<R::StreamInto, R::Error>> + Send>>,
	sink: Pin<Box<dyn Sink<R::SinkInto, Error = R::Error> + Send>>,
}

impl<R: Reframe> Reframed<R> {
	pub fn new(source: impl Stream<Item = Result<R::StreamFrom, R::Error>> + Sink<R::SinkFrom, Error = R::Error> + Send + 'static) -> Reframed<R> {
		let (sink, stream) = source.split();
		Self::from_parts(stream, sink)
	}

	pub fn from_parts(stream: impl Stream<Item = Result<R::StreamFrom, R::Error>> + Send + 'static, sink: impl Sink<R::SinkFrom, Error = R::Error> + Send + 'static) -> Reframed<R> {
		let Source { stream, sink } = R::reframe(Source::new(stream, sink));
		Reframed { stream, sink }
	}
}

impl<R: Reframe> Stream for Reframed<R> {
	type Item = Result<R::StreamInto, R::Error>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		Pin::new(&mut self.get_mut().stream).poll_next(cx)
	}
}

impl<R: Reframe> Sink<R::SinkInto> for Reframed<R> {
	type Error = R::Error;

	fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_ready(cx)
	}

	fn start_send(self: Pin<&mut Self>, item: R::SinkInto) -> Result<(), Self::Error> {
		Pin::new(&mut Pin::get_mut(self).sink).start_send(item)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_flush(cx)
	}

	fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_close(cx)
	}
}
