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

mod send_wrapper;

use std::future::IntoFuture;
use std::ops::Range;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::FutureExt;
use futures::StreamExt;
use futures::TryStreamExt;
use object_store::path::Path;
use object_store::GetOptions;
use object_store::GetResult;
use object_store::GetResultPayload;
use object_store::ListResult;
use object_store::MultipartUpload;
use object_store::ObjectMeta;
use object_store::ObjectStore;
use object_store::PutMultipartOpts;
use object_store::PutOptions;
use object_store::PutPayload;
use object_store::PutResult;
use object_store::Result;
use opendal::Entry;
use opendal::Metadata;
use opendal::Metakey;
use opendal::Operator;
use send_wrapper::IntoSendFuture;
use send_wrapper::IntoSendStream;

#[derive(Debug)]
pub struct OpendalStore {
    inner: Operator,
}

impl OpendalStore {
    /// Create OpendalStore by given Operator.
    pub fn new(op: Operator) -> Self {
        Self { inner: op }
    }
}

impl std::fmt::Display for OpendalStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenDAL({:?})", self.inner)
    }
}

#[async_trait]
impl ObjectStore for OpendalStore {
    async fn put(&self, location: &Path, bytes: PutPayload) -> Result<PutResult> {
        let bytes: Bytes = bytes.into();
        self.inner
            .write(location.as_ref(), bytes)
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;
        Ok(PutResult {
            e_tag: None,
            version: None,
        })
    }

    async fn put_opts(
        &self,
        _location: &Path,
        _bytes: PutPayload,
        _opts: PutOptions,
    ) -> Result<PutResult> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "put_opts is not implemented so far",
            )),
        })
    }

    async fn put_multipart(&self, _location: &Path) -> Result<Box<dyn MultipartUpload>> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "put_multipart is not implemented so far",
            )),
        })
    }

    async fn put_multipart_opts(
        &self,
        _location: &Path,
        _opts: PutMultipartOpts,
    ) -> Result<Box<dyn MultipartUpload>> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "put_multipart_opts is not implemented so far",
            )),
        })
    }

    async fn get(&self, location: &Path) -> Result<GetResult> {
        let meta = self
            .inner
            .stat(location.as_ref())
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;

        let meta = ObjectMeta {
            location: location.clone(),
            last_modified: meta.last_modified().unwrap_or_default(),
            size: meta.content_length() as usize,
            e_tag: meta.etag().map(|x| x.to_string()),
            version: meta.version().map(|x| x.to_string()),
        };
        let r = self
            .inner
            .reader(location.as_ref())
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;

        let stream = r
            .into_bytes_stream(0..meta.size as u64)
            .await
            .map_err(|err| object_store::Error::Generic {
                store: "IoError",
                source: Box::new(err),
            })?
            .into_send()
            .map_err(|err| object_store::Error::Generic {
                store: "IoError",
                source: Box::new(err),
            });

        Ok(GetResult {
            payload: GetResultPayload::Stream(Box::pin(stream)),
            range: 0..meta.size,
            meta,
            attributes: Default::default(),
        })
    }

    async fn get_opts(&self, _location: &Path, _options: GetOptions) -> Result<GetResult> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "get_opts is not implemented so far",
            )),
        })
    }

    async fn get_range(&self, location: &Path, range: Range<usize>) -> Result<Bytes> {
        let bs = self
            .inner
            .read_with(location.as_ref())
            .range(range.start as u64..range.end as u64)
            .into_future()
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;

        Ok(bs.to_bytes())
    }

    async fn head(&self, location: &Path) -> Result<ObjectMeta> {
        let meta = self
            .inner
            .stat(location.as_ref())
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;

        Ok(ObjectMeta {
            location: location.clone(),
            last_modified: meta.last_modified().unwrap_or_default(),
            size: meta.content_length() as usize,
            e_tag: meta.etag().map(|x| x.to_string()),
            version: meta.version().map(|x| x.to_string()),
        })
    }

    async fn delete(&self, location: &Path) -> Result<()> {
        self.inner
            .delete(location.as_ref())
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, location.as_ref()))?;

        Ok(())
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'_, Result<ObjectMeta>> {
        // object_store `Path` always removes trailing slash
        // need to add it back
        let path = prefix.map_or("".into(), |x| format!("{}/", x));

        let fut = async move {
            let stream = self
                .inner
                .lister_with(&path)
                .metakey(Metakey::ContentLength | Metakey::LastModified)
                .recursive(true)
                .await
                .map_err(|err| format_object_store_error(err, &path))?;

            let stream = stream.then(|res| async {
                let entry = res.map_err(|err| format_object_store_error(err, ""))?;
                let meta = entry.metadata();

                Ok(format_object_meta(entry.path(), meta))
            });
            Ok::<_, object_store::Error>(stream)
        };

        fut.into_stream().try_flatten().into_send().boxed()
    }

    fn list_with_offset(
        &self,
        prefix: Option<&Path>,
        offset: &Path,
    ) -> BoxStream<'_, Result<ObjectMeta>> {
        let path = prefix.map_or("".into(), |x| format!("{}/", x));
        let offset = offset.clone();

        let fut = async move {
            let fut = if self.inner.info().full_capability().list_with_start_after {
                self.inner
                    .lister_with(&path)
                    .start_after(offset.as_ref())
                    .metakey(Metakey::ContentLength | Metakey::LastModified)
                    .recursive(true)
                    .into_future()
                    .into_send()
                    .await
                    .map_err(|err| format_object_store_error(err, &path))?
                    .then(try_format_object_meta)
                    .into_send()
                    .boxed()
            } else {
                self.inner
                    .lister_with(&path)
                    .metakey(Metakey::ContentLength | Metakey::LastModified)
                    .recursive(true)
                    .into_future()
                    .into_send()
                    .await
                    .map_err(|err| format_object_store_error(err, &path))?
                    .try_filter(move |entry| futures::future::ready(entry.path() > offset.as_ref()))
                    .then(try_format_object_meta)
                    .into_send()
                    .boxed()
            };
            Ok::<_, object_store::Error>(fut)
        };

        fut.into_stream().into_send().try_flatten().boxed()
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        let path = prefix.map_or("".into(), |x| format!("{}/", x));
        let mut stream = self
            .inner
            .lister_with(&path)
            .metakey(Metakey::Mode | Metakey::ContentLength | Metakey::LastModified)
            .into_future()
            .into_send()
            .await
            .map_err(|err| format_object_store_error(err, &path))?
            .into_send();

        let mut common_prefixes = Vec::new();
        let mut objects = Vec::new();

        while let Some(res) = stream.next().into_send().await {
            let entry = res.map_err(|err| format_object_store_error(err, ""))?;
            let meta = entry.metadata();

            if meta.is_dir() {
                common_prefixes.push(entry.path().into());
            } else {
                objects.push(format_object_meta(entry.path(), meta));
            }
        }

        Ok(ListResult {
            common_prefixes,
            objects,
        })
    }

    async fn copy(&self, _from: &Path, _to: &Path) -> Result<()> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "copy is not implemented so far",
            )),
        })
    }

    async fn rename(&self, _from: &Path, _to: &Path) -> Result<()> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "rename is not implemented so far",
            )),
        })
    }

    async fn copy_if_not_exists(&self, _from: &Path, _to: &Path) -> Result<()> {
        Err(object_store::Error::NotSupported {
            source: Box::new(opendal::Error::new(
                opendal::ErrorKind::Unsupported,
                "copy_if_not_exists is not implemented so far",
            )),
        })
    }
}

fn format_object_store_error(err: opendal::Error, path: &str) -> object_store::Error {
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
        kind => object_store::Error::Generic {
            store: kind.into_static(),
            source: Box::new(err),
        },
    }
}

fn format_object_meta(path: &str, meta: &Metadata) -> ObjectMeta {
    let version = match meta.metakey().contains(Metakey::Version) {
        true => meta.version().map(|x| x.to_string()),
        false => None,
    };

    let e_tag = match meta.metakey().contains(Metakey::Etag) {
        true => meta.etag().map(|x| x.to_string()),
        false => None,
    };

    ObjectMeta {
        location: path.into(),
        last_modified: meta.last_modified().unwrap_or_default(),
        size: meta.content_length() as usize,
        e_tag,
        version,
    }
}

async fn try_format_object_meta(res: Result<Entry, opendal::Error>) -> Result<ObjectMeta> {
    let entry = res.map_err(|err| format_object_store_error(err, ""))?;
    let meta = entry.metadata();

    Ok(format_object_meta(entry.path(), meta))
}

// Make sure `send_wrapper` works as expected
#[cfg(all(feature = "send_wrapper", target_arch = "wasm32"))]
mod assert_send {
    use object_store::ObjectStore;

    #[allow(dead_code)]
    fn assert_send<T: Send>(_: T) {}

    #[allow(dead_code)]
    fn assertion() {
        let op = super::Operator::new(opendal::services::Memory::default())
            .unwrap()
            .finish();
        let store = super::OpendalStore::new(op);
        assert_send(store.put(&"test".into(), bytes::Bytes::new()));
        assert_send(store.get(&"test".into()));
        assert_send(store.get_range(&"test".into(), 0..1));
        assert_send(store.head(&"test".into()));
        assert_send(store.delete(&"test".into()));
        assert_send(store.list(None));
        assert_send(store.list_with_offset(None, &"test".into()));
        assert_send(store.list_with_delimiter(None));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use object_store::path::Path;
    use object_store::ObjectStore;
    use opendal::services;

    use super::*;

    async fn create_test_object_store() -> Arc<dyn ObjectStore> {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let object_store = Arc::new(OpendalStore::new(op));

        let path: Path = "data/test.txt".into();
        let bytes = Bytes::from_static(b"hello, world!");
        object_store.put(&path, bytes.into()).await.unwrap();

        let path: Path = "data/nested/test.txt".into();
        let bytes = Bytes::from_static(b"hello, world! I am nested.");
        object_store.put(&path, bytes.into()).await.unwrap();

        object_store
    }

    #[tokio::test]
    async fn test_basic() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let object_store: Arc<dyn ObjectStore> = Arc::new(OpendalStore::new(op));

        // Retrieve a specific file
        let path: Path = "data/test.txt".into();

        let bytes = Bytes::from_static(b"hello, world!");
        object_store.put(&path, bytes.clone().into()).await.unwrap();

        let meta = object_store.head(&path).await.unwrap();

        assert_eq!(meta.size, 13);

        assert_eq!(
            object_store
                .get(&path)
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap(),
            bytes
        );
    }

    #[tokio::test]
    async fn test_list() {
        let object_store = create_test_object_store().await;
        let path: Path = "data/".into();
        let results = object_store.list(Some(&path)).collect::<Vec<_>>().await;
        assert_eq!(results.len(), 2);
        let mut locations = results
            .iter()
            .map(|x| x.as_ref().unwrap().location.as_ref())
            .collect::<Vec<_>>();

        let expected_files = vec![
            (
                "data/nested/test.txt",
                Bytes::from_static(b"hello, world! I am nested."),
            ),
            ("data/test.txt", Bytes::from_static(b"hello, world!")),
        ];

        let expected_locations = expected_files.iter().map(|x| x.0).collect::<Vec<&str>>();

        locations.sort();
        assert_eq!(locations, expected_locations);

        for (location, bytes) in expected_files {
            let path: Path = location.into();
            assert_eq!(
                object_store
                    .get(&path)
                    .await
                    .unwrap()
                    .bytes()
                    .await
                    .unwrap(),
                bytes
            );
        }
    }

    #[tokio::test]
    async fn test_list_with_delimiter() {
        let object_store = create_test_object_store().await;
        let path: Path = "data/".into();
        let result = object_store.list_with_delimiter(Some(&path)).await.unwrap();
        assert_eq!(result.objects.len(), 1);
        assert_eq!(result.common_prefixes.len(), 1);
        assert_eq!(result.objects[0].location.as_ref(), "data/test.txt");
        assert_eq!(result.common_prefixes[0].as_ref(), "data/nested");
    }

    #[tokio::test]
    async fn test_list_with_offset() {
        let object_store = create_test_object_store().await;
        let path: Path = "data/".into();
        let offset: Path = "data/nested/test.txt".into();
        let result = object_store
            .list_with_offset(Some(&path), &offset)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].as_ref().unwrap().location.as_ref(),
            "data/test.txt"
        );
    }
}
