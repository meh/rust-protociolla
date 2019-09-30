use std::{future::Future, pin::Pin, task::{Context, Poll}};
use futures::{stream::{Stream, StreamExt}, sink::Sink};
use tokio::sync::mpsc::{self, channel, Receiver, Sender};

pub trait Reframe {
  type Input: Send + 'static;
  type Output: Send + 'static;
  type Error: Send + 'static;

  fn stream(stream: Pin<Box<dyn Stream<Item = Result<Self::Input, Self::Error>> + Send>>)
    -> Pin<Box<dyn Stream<Item = Result<Self::Output, Self::Error>> + Send>>;

  fn sink(sink: Pin<Box<dyn Sink<Self::Input, Error = Self::Error> + Send>>)
    -> Pin<Box<dyn Sink<Self::Output, Error = Self::Error> + Send>>;
}

pub struct Reframed<R: Reframe> {
  stream: Pin<Box<dyn Stream<Item = Result<R::Output, R::Error>> + Send>>,
  sink: Pin<Box<dyn Sink<R::Output, Error = R::Error> + Send>>,
}

impl<R: Reframe> Reframed<R> {
  pub fn new(source: impl Stream<Item = Result<R::Input, R::Error>> + Sink<R::Input, Error = R::Error> + Send + 'static) -> Reframed<R> {
    let (sink, stream) = source.split();

    Reframed {
      stream: R::stream(Box::pin(stream)),
      sink: R::sink(Box::pin(sink))
    }
  }

  pub fn from_parts(stream: Pin<Box<dyn Stream<Item = Result<R::Output, R::Error>> + Send>>, sink: Pin<Box<dyn Sink<R::Output, Error = R::Error> + Send>>) -> Reframed<R> {
    Reframed { stream, sink }
  }
}

impl<R: Reframe> Stream for Reframed<R> {
  type Item = Result<R::Output, R::Error>;

  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    Pin::new(&mut self.get_mut().stream).poll_next(cx)
  }
}

impl<R: Reframe> Sink<R::Output> for Reframed<R> {
  type Error = R::Error;

	fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_ready(cx)
	}

	fn start_send(self: Pin<&mut Self>, item: R::Output) -> Result<(), Self::Error> {
		Pin::new(&mut Pin::get_mut(self).sink).start_send(item)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_flush(cx)
	}

	fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Pin::new(&mut Pin::get_mut(self).sink).poll_close(cx)
	}
}

pub fn stream<R: Reframe, F, O>(func: F) -> Receiver<Result<R::Output, R::Error>>
	where F: FnOnce(mpsc::Sender<Result<R::Output, R::Error>>) -> O,
	      O: Future<Output = ()> + Send + 'static
{
	let (tx, rx) = channel(16);
	tokio::spawn(func(tx));
	rx
}

pub fn sink<R: Reframe, F, O>(func: F) -> Sender<R::Output>
	where F: FnOnce(mpsc::Receiver<R::Output>) -> O,
	      O: Future<Output = ()> + Send + 'static
{
	let (tx, rx) = channel(16);
	tokio::spawn(func(rx));
	tx
}
