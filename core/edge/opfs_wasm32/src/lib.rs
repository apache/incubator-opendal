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

#![cfg(target_arch = "wasm32")]
#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use opendal::EntryMode;
    use opendal::ErrorKind;
    use opendal::Operator;
    use opendal::services::Opfs;
    use opendal::services::OpfsConfig;
    use wasm_bindgen_test::wasm_bindgen_test;
    use wasm_bindgen_test::wasm_bindgen_test_configure; // Required for Next()

    macro_rules! console_log {
        ($($arg:tt)*) => {
            web_sys::console::log_1(&format!($($arg)*).into())
        };
    }

    wasm_bindgen_test_configure!(run_in_browser);

    fn new_operator() -> Operator {
        Operator::from_config(OpfsConfig::default())
            .expect("failed to create opfs operator")
            .finish()
    }

    #[wasm_bindgen_test]
    async fn test_create_directory_handle() {
        let op = new_operator();
        op.create_dir("/dir/").await.expect("directory");
        op.create_dir("/dir///").await.expect("directory");
        op.create_dir("/dir:/").await.expect("directory");
        op.create_dir("/dir<>/").await.expect("directory");
        assert_eq!(
            op.create_dir("/a/b/../x/y/z/").await.unwrap_err().kind(),
            ErrorKind::Unexpected
        );
        // this works on Chrome, but fails on macOS
        // assert_eq!(op.create_dir("/dir\0/").await.unwrap_err().kind(), ErrorKind::Unexpected);
    }

    #[wasm_bindgen_test]
    async fn test_create_directory_handle_with_root_and_list() {
        {
            // create files and dirs
            let op_rooted =
                Operator::new(Opfs::default().root("/myapp/subdir1/subdir2/")).expect("config");
            op_rooted
                .write("subdir3/somefile", "content")
                .await
                .expect("write under root");

            let stat_rooted = op_rooted.stat("subdir3/somefile").await.expect("stat");

            let op = new_operator();
            let stat = op
                .stat("/myapp/subdir1/subdir2/subdir3/somefile")
                .await
                .expect("stat");
            assert_eq!(stat_rooted, stat_rooted);
            let stat = op
                .stat("myapp/subdir1/subdir2/subdir3/somefile")
                .await
                .expect("stat");
            assert_eq!(stat_rooted, stat_rooted);
        }

        {
            // simple list
            let op = new_operator();
            let mut entries = op.lister("").await.expect("list");
            while let Some(entry) = entries.try_next().await.expect("next") {
                console_log!("entry: {} {:?}", entry.path(), entry.metadata().mode());
            }
        }

        // list test added here so we are sure there are dirs and files to list
        {
            // simple list
            let op = new_operator();
            let mut entries = op.lister("myapp/").await.expect("list");
            while let Some(entry) = entries.try_next().await.expect("next") {
                console_log!("entry: {} {:?}", entry.path(), entry.metadata().mode());
            }
        }

        {
            // recursive list
            let op = new_operator();
            let mut entries = op
                .lister_with("")
                .recursive(true)
                .await
                .expect("recursive list");
            while let Some(entry) = entries.try_next().await.expect("next") {
                console_log!("rec entry: {} {:?}", entry.path(), entry.metadata().mode());
            }
        }
    }

    #[wasm_bindgen_test]
    async fn test_write() {
        let op = new_operator();

        // this does not even go to OPFS backend, short-circuited
        assert_eq!(
            op.write("/", "should_not_work").await.unwrap_err().kind(),
            ErrorKind::IsADirectory
        );
    }

    #[wasm_bindgen_test]
    async fn test_write_read_simple() {
        let path = "/test_file";
        let content = "Content of the file to write";
        {
            let op = new_operator();
            let meta = op.write(path, content).await.expect("write");
            console_log!("{:?}", meta);
            assert_eq!(meta.content_length(), content.len() as u64);

            // This is None - we have to use stat
            assert!(meta.last_modified().is_none());

            let stat = op.stat(path).await.expect("stat");
            console_log!("stat = {:?}", stat);
            assert_eq!(stat.mode(), EntryMode::FILE);
            assert_eq!(stat.content_length(), content.len() as u64);
            assert!(stat.last_modified().is_some());
        }

        {
            // read back and compare
            let op = new_operator();
            let buffer = op.read(path).await.expect("read");
            console_log!("read = {:?}", buffer);
            assert_eq!(buffer.to_bytes(), content.as_bytes());
            op.delete(path).await.expect("delete");
        }
    }

    #[wasm_bindgen_test]
    async fn test_write_write_twice_same_file() {
        let op = new_operator();
        let content = "Content of the file to write";
        let path = "/test_file";
        let meta = op.write(path, content).await.expect("write");
        let meta = op.write(path, content).await.expect("write");
        assert_eq!(meta.content_length(), content.len() as u64);
        assert!(meta.last_modified().is_none());
        op.delete(path).await.expect("delete");
    }

    #[wasm_bindgen_test]
    async fn test_write_like_append_three_times() {
        let op = new_operator();
        let content = "Content of the file to write";
        let path = "/test_file_write_multiple";
        let mut w = op.writer(path).await.expect("writer");
        w.write(content).await.expect("write");
        w.write(content).await.expect("write");
        w.write(content).await.expect("write");
        let meta = w.close().await.expect("close");
        let expected_file_size = (content.len() as u64) * 3;
        assert_eq!(meta.content_length(), expected_file_size);
        assert!(meta.last_modified().is_none());
        let stat = op.stat(path).await.expect("stat");
        assert_eq!(stat.content_length(), expected_file_size);
        op.delete(path).await.expect("delete");
    }

    #[wasm_bindgen_test]
    async fn test_write_large_file_quota() {
        // you can simulate a lower disk space in Chrome
        let op = new_operator();
        let path = "big_file";
        let mut w = op.writer(path).await.expect("writer");
        let chunk = vec![0u8; 1024 * 1024]; // 1MB
        for _ in 0..1024 {
            let res = w.write(chunk.clone()).await;
            match res {
                Ok(()) => (),
                Err(e) => {
                    // OPFS filled up (you can simulate this in Chrome by setting a lower limit)
                    // parse_js_error: JsValue(TypeError: Cannot close a ERRORED writable stream
                    // TypeError: Cannot close a ERRORED writable stream
                    console_log!("got {e:?}");
                    console_log!("message = {}", e.message());
                    assert_eq!(e.kind(), ErrorKind::Unexpected);
                    return;
                }
            }
        }

        let expected_file_size = 1024 * 1024 * 1024; // 1GB
        let meta = w.close().await.expect("close");
        assert_eq!(meta.content_length(), expected_file_size);
        let stat = op.stat(path).await.expect("stat");
        assert_eq!(stat.content_length(), expected_file_size);

        {
            // read and compare
            let op = new_operator();
            let buffer = op.read(path).await.expect("read");
            assert_eq!(buffer.to_bytes().len(), expected_file_size as usize);
        }
        op.delete(path).await.expect("delete");
    }

    #[wasm_bindgen_test]
    async fn test_write_and_read_with_range() {
        let op = new_operator();
        let path = "numbers.txt";
        let content = "0123456789";
        let meta = op.write(path, content).await.expect("write");
        let buffer = op.read_with(path).range(3..5).await.expect("read");
        assert_eq!(buffer.to_bytes(), "34".as_bytes());
        op.delete(path).await.expect("delete");
    }
}
