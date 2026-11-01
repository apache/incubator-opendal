// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use futures::Stream;
use object_store::ObjectMeta;
use opendal::Metadata;
use std::future::IntoFuture;

use crate::timestamp_to_datetime;
/// Conditionally add the `Send` marker trait for the wrapped type.
/// Only take effect when the `send_wrapper` feature is enabled.
#[cfg(not(feature = "send_wrapper"))]
use noop_wrapper::NoopWrapper as SendWrapper;
#[cfg(feature = "send_wrapper")]
use send_wrapper::SendWrapper;

/// Format `opendal::Error` to `object_store::Error`.
pub fn format_object_store_error(err: opendal::Error, path: &str) -> object_store::Error {
    use opendal::ErrorKind;
    match err.kind() {
        ErrorKind::NotFound => object_store::Error::NotFound {
            path: path.to_string(),
            source: Box::new(err),
        },
        ErrorKind::Unsupported => object_store::Error::NotSupported {
            source: Box::new(err),
        },
        ErrorKind::AlreadyExists => object_store::Error::AlreadyExists {
            path: path.to_string(),
            source: Box::new(err),
        },
        ErrorKind::ConditionNotMatch => object_store::Error::Precondition {
            path: path.to_string(),
            source: Box::new(err),
        },
        kind => object_store::Error::Generic {
            store: kind.into_static(),
            source: Box::new(err),
        },
    }
}

/// Format `opendal::Metadata` to `object_store::ObjectMeta`.
pub fn format_object_meta(path: &str, meta: &Metadata) -> ObjectMeta {
    ObjectMeta {
        location: path.into(),
        last_modified: meta
            .last_modified()
            .and_then(timestamp_to_datetime)
            .unwrap_or_default(),
        size: meta.content_length(),
        e_tag: meta.etag().map(|x| x.to_string()),
        version: meta.version().map(|x| x.to_string()),
    }
}

/// Make given future `Send`.
pub trait IntoSendFuture {
    type Output;

    fn into_send(self) -> Self::Output;
}

impl<T> IntoSendFuture for T
where
    T: IntoFuture,
{
    type Output = SendWrapper<T::IntoFuture>;

    fn into_send(self) -> Self::Output {
        SendWrapper::new(self.into_future())
    }
}

/// Make given Stream `Send`.
pub trait IntoSendStream {
    type Output;

    fn into_send(self) -> Self::Output;
}

impl<T> IntoSendStream for T
where
    T: Stream,
{
    type Output = SendWrapper<T>;
    fn into_send(self) -> Self::Output {
        SendWrapper::new(self)
    }
}

#[cfg(not(feature = "send_wrapper"))]
mod noop_wrapper {
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;

    use futures::Future;
    use futures::Stream;
    use pin_project::pin_project;

    #[pin_project]
    pub struct NoopWrapper<T> {
        #[pin]
        item: T,
    }

    impl<T> Future for NoopWrapper<T>
    where
        T: Future,
    {
        type Output = T::Output;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.project();
            this.item.poll(cx)
        }
    }

    impl<T> Stream for NoopWrapper<T>
    where
        T: Stream,
    {
        type Item = T::Item;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = self.project();
            this.item.poll_next(cx)
        }
    }

    impl<T> NoopWrapper<T> {
        pub fn new(item: T) -> Self {
            Self { item }
        }
    }
}
