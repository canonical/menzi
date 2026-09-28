use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use menzi_common::{MenziError, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::Event;

pub type EventStream<T> = Pin<Box<dyn Stream<Item = Result<Event<T>>> + Send>>;

#[derive(Clone)]
pub struct NatsEventBus {
    jetstream: async_nats::jetstream::Context,
}

impl NatsEventBus {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = async_nats::connect(url).await.map_err(bus_error)?;
        let jetstream = async_nats::jetstream::new(client);
        Ok(Self { jetstream })
    }

    pub async fn ensure_stream(&self, name: &str, subjects: &[&str]) -> Result<()> {
        self.jetstream
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: name.to_string(),
                subjects: subjects.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            })
            .await
            .map_err(bus_error)?;
        Ok(())
    }

    pub async fn publish<T: Serialize>(&self, subject: &str, event: &Event<T>) -> Result<()> {
        let payload = serde_json::to_vec(event).map_err(bus_error)?;
        self.jetstream
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(bus_error)?;
        Ok(())
    }

    pub async fn subscribe<T: DeserializeOwned + Send + 'static>(
        &self,
        stream_name: &str,
        subject: &str,
    ) -> Result<EventStream<T>> {
        let stream = self
            .jetstream
            .get_stream(stream_name)
            .await
            .map_err(bus_error)?;
        let consumer = stream
            .get_or_create_consumer(
                stream_name,
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(stream_name.to_string()),
                    filter_subject: subject.to_string(),
                    ..Default::default()
                },
            )
            .await
            .map_err(bus_error)?;
        let messages = consumer.messages().await.map_err(bus_error)?;
        Ok(Box::pin(messages.filter_map(|message| async move {
            let Ok(message) = message else {
                return None;
            };
            let event: Event<T> = match serde_json::from_slice(&message.payload) {
                Ok(event) => event,
                Err(_) => return None,
            };
            let _ = message.ack().await;
            Some(Ok(event))
        })))
    }
}

fn bus_error(err: impl std::fmt::Display) -> MenziError {
    MenziError::Bus(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_error_maps_to_menzi_error() {
        let err = bus_error("boom");
        assert_eq!(format!("{}", err), "event bus: boom");
    }
}
