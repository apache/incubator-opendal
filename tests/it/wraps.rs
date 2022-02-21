// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::SeekFrom;
use std::str::from_utf8;

use futures::io::copy;

use futures::io::Cursor;
use futures::AsyncReadExt;
use futures::AsyncSeekExt;
use futures::StreamExt;
use opendal::readers::CallbackReader;
use opendal::readers::ReaderStream;

use opendal::services::fs;
use opendal::BoxedAsyncRead;
use opendal::Operator;

#[tokio::test]
async fn reader_stream() {
    let reader = BoxedAsyncRead::new(Box::new(Cursor::new("Hello, world!")));
    let mut s = ReaderStream::new(reader);

    let mut bs = Vec::new();
    while let Some(chunk) = s.next().await {
        bs.extend_from_slice(&chunk.unwrap());
    }

    assert_eq!(&bs[..], "Hello, world!".to_string().as_bytes());
}

#[tokio::test]
async fn callback_reader() {
    let mut size = 0;

    let reader = CallbackReader::new(
        BoxedAsyncRead::new(Box::new(Cursor::new("Hello, world!"))),
        |n| size += n,
    );

    let mut bs = Vec::new();
    let n = copy(reader, &mut bs).await.unwrap();

    assert_eq!(size, 13);
    assert_eq!(n, 13);
}

#[tokio::test]
async fn test_seekable_reader() {
    let f = Operator::new(fs::Backend::build().finish().await.unwrap());

    let path = format!("/tmp/{}", uuid::Uuid::new_v4());

    // Create a test file.
    let x = f
        .object(&path)
        .write_bytes("Hello, world!".to_string().into_bytes())
        .await
        .unwrap();
    assert_eq!(x, 13);

    let mut r = f.object(&path).stateful_read().await.unwrap();

    // Seek to offset 3.
    let n = r.seek(SeekFrom::Start(3)).await.expect("seek");
    assert_eq!(n, 3);

    // Read only one byte.
    let mut bs = Vec::new();
    bs.resize(1, 0);
    let n = r.read(&mut bs).await.expect("read");
    assert_eq!("l", from_utf8(&bs).unwrap());
    assert_eq!(n, 1);
    let n = r.seek(SeekFrom::Current(0)).await.expect("seek");
    assert_eq!(n, 4);

    // Seek to end.
    let n = r.seek(SeekFrom::End(-1)).await.expect("seek");
    assert_eq!(n, 12);

    // Read only one byte.
    let mut bs = Vec::new();
    bs.resize(1, 0);
    let n = r.read(&mut bs).await.expect("read");
    assert_eq!("!", from_utf8(&bs).unwrap());
    assert_eq!(n, 1);
    let n = r.seek(SeekFrom::Current(0)).await.expect("seek");
    assert_eq!(n, 13);
}
