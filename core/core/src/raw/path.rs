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

use crate::*;
use std::hash::{BuildHasher, Hasher};

/// build_abs_path will build an absolute path with root.
///
/// # Rules
///
/// - Input root MUST be the format like `/abc/def/`
/// - Output will be the format like `path/to/root/path`.
pub fn build_abs_path(root: &str, path: &str) -> String {
    debug_assert!(root.starts_with('/'), "root must start with /");
    debug_assert!(root.ends_with('/'), "root must end with /");

    let p = root[1..].to_string();

    if path == "/" {
        p
    } else {
        debug_assert!(!path.starts_with('/'), "path must not start with /");
        p + path
    }
}

/// build_rooted_abs_path will build an absolute path with root.
///
/// # Rules
///
/// - Input root MUST be the format like `/abc/def/`
/// - Output will be the format like `/path/to/root/path`.
pub fn build_rooted_abs_path(root: &str, path: &str) -> String {
    debug_assert!(root.starts_with('/'), "root must start with /");
    debug_assert!(root.ends_with('/'), "root must end with /");

    let p = root.to_string();

    if path == "/" {
        p
    } else {
        debug_assert!(!path.starts_with('/'), "path must not start with /");
        p + path
    }
}

/// build_rel_path will build a relative path towards root.
///
/// # Rules
///
/// - Input root MUST be the format like `/abc/def/`
/// - Input path MUST start with root like `/abc/def/path/to/file`
/// - Output will be the format like `path/to/file`.
pub fn build_rel_path(root: &str, path: &str) -> String {
    debug_assert!(root != path, "get rel path with root is invalid");

    if path.starts_with('/') {
        debug_assert!(
            path.starts_with(root),
            "path {path} doesn't start with root {root}"
        );
        path[root.len()..].to_string()
    } else {
        debug_assert!(
            path.starts_with(&root[1..]),
            "path {path} doesn't start with root {root}"
        );
        path[root.len() - 1..].to_string()
    }
}

/// Make sure all operation are constructed by normalized path:
///
/// - Path endswith `/` means it's a dir path.
/// - Otherwise, it's a file path.
///
/// # Normalize Rules
///
/// - All whitespace will be trimmed: ` abc/def ` => `abc/def`
/// - All leading / will be trimmed: `///abc` => `abc`
/// - Internal // will be replaced by /: `abc///def` => `abc/def`
/// - Empty path will be `/`: `` => `/`
pub fn normalize_path(path: &str) -> String {
    // - all whitespace has been trimmed.
    // - all leading `/` has been trimmed.
    let path = path.trim().trim_start_matches('/');

    // Fast line for empty path.
    if path.is_empty() {
        return "/".to_string();
    }

    let has_trailing = path.ends_with('/');

    let mut p = path
        .split('/')
        .filter(|v| !v.is_empty())
        .collect::<Vec<&str>>()
        .join("/");

    // Append trailing back if input path is endswith `/`.
    if has_trailing {
        p.push('/');
    }

    p
}

/// Make sure root is normalized to style like `/abc/def/`.
///
/// # Normalize Rules
///
/// - All whitespace will be trimmed: ` abc/def ` => `abc/def`
/// - All leading / will be trimmed: `///abc` => `abc`
/// - Internal // will be replaced by /: `abc///def` => `abc/def`
/// - Empty path will be `/`: `` => `/`
/// - Add leading `/` if not starts with: `abc/` => `/abc/`
/// - Add trailing `/` if not ends with: `/abc` => `/abc/`
///
/// Finally, we will get path like `/path/to/root/`.
pub fn normalize_root(v: &str) -> String {
    let mut v = v
        .split('/')
        .filter(|v| !v.is_empty())
        .collect::<Vec<&str>>()
        .join("/");
    if !v.starts_with('/') {
        v.insert(0, '/');
    }
    if !v.ends_with('/') {
        v.push('/')
    }
    v
}

/// Get basename from path.
pub fn get_basename(path: &str) -> &str {
    // Handle root case
    if path == "/" {
        return "/";
    }

    // Handle file case
    if !path.ends_with('/') {
        return path
            .split('/')
            .next_back()
            .expect("file path without name is invalid");
    }

    // The idx of second `/` if path in reserve order.
    // - `abc/` => `None`
    // - `abc/def/` => `Some(3)`
    let idx = path[..path.len() - 1].rfind('/').map(|v| v + 1);

    match idx {
        Some(v) => {
            let (_, name) = path.split_at(v);
            name
        }
        None => path,
    }
}

/// Get parent from path.
pub fn get_parent(path: &str) -> &str {
    if path == "/" {
        return "/";
    }

    if !path.ends_with('/') {
        // The idx of first `/` if path in reserve order.
        // - `abc` => `None`
        // - `abc/def` => `Some(3)`
        let idx = path.rfind('/');

        return match idx {
            Some(v) => {
                let (parent, _) = path.split_at(v + 1);
                parent
            }
            None => "/",
        };
    }

    // The idx of second `/` if path in reserve order.
    // - `abc/` => `None`
    // - `abc/def/` => `Some(3)`
    let idx = path[..path.len() - 1].rfind('/').map(|v| v + 1);

    match idx {
        Some(v) => {
            let (parent, _) = path.split_at(v);
            parent
        }
        None => "/",
    }
}

// Sets the size of random generated postfix for random file names
const RANDOM_TMP_PATH_POSTFIX_LENGTH: usize = 8;
// Allowed characters for choices in a random-generated char
const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const CHARS_LENGTH: u64 = CHARS.len() as u64;

/// Build a temporary path of a file path.
///
/// `build_tmp_path_of` appends a dot following a random generated postfix.
/// Don't use it with a path to a folder.
#[inline]
pub fn build_tmp_path_of(path: &str) -> String {
    let name = get_basename(path);
    let mut buf = String::with_capacity(name.len() + RANDOM_TMP_PATH_POSTFIX_LENGTH);
    buf.push_str(name);
    buf.push('.');

    // Uses `std` for the random number generator instead of external crates.
    //
    // `RandomState::new` builds a hasher that generates a different random sequence each time.
    // Calling `RandomState::new` each time produces a `RandomState` from
    // a per-thread pseudo seed pools that Rust manages.
    // The default hasher, `SipHasher13`, has some notable properties:
    //
    // 1. `fastrand` is roughly 10x faster than `SipHasher13`, but it adds an extra dependency.
    // 2. While `fastrand` is faster, `SipHasher13` is fast enough for our needs since
    //    we're only generating a few characters.
    // 3. This is not a cryptographically secure pseudorandom number generator (CSPRNG).
    //
    // If we need stronger randomness in the future, we can:
    // 1. Increase the output length.
    // 2. Use the `getrandom` crate to source randomness from the OS.
    for _ in 0..RANDOM_TMP_PATH_POSTFIX_LENGTH {
        let random = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        let choice: usize = (random % CHARS_LENGTH).try_into().unwrap();
        buf.push(CHARS[choice] as char);
    }

    buf
}

/// Validate given path is match with given EntryMode.
#[inline]
pub fn validate_path(path: &str, mode: EntryMode) -> bool {
    debug_assert!(!path.is_empty(), "input path should not be empty");

    match mode {
        EntryMode::FILE => !path.ends_with('/'),
        EntryMode::DIR => path.ends_with('/'),
        EntryMode::Unknown => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        let cases = vec![
            ("file path", "abc", "abc"),
            ("dir path", "abc/", "abc/"),
            ("empty path", "", "/"),
            ("root path", "/", "/"),
            ("root path with extra /", "///", "/"),
            ("abs file path", "/abc/def", "abc/def"),
            ("abs dir path", "/abc/def/", "abc/def/"),
            ("abs file path with extra /", "///abc/def", "abc/def"),
            ("abs dir path with extra /", "///abc/def/", "abc/def/"),
            ("file path contains ///", "abc///def", "abc/def"),
            ("dir path contains ///", "abc///def///", "abc/def/"),
            ("file with whitespace", "abc/def   ", "abc/def"),
        ];

        for (name, input, expect) in cases {
            assert_eq!(normalize_path(input), expect, "{name}")
        }
    }

    #[test]
    fn test_normalize_root() {
        let cases = vec![
            ("dir path", "abc/", "/abc/"),
            ("empty path", "", "/"),
            ("root path", "/", "/"),
            ("root path with extra /", "///", "/"),
            ("abs dir path", "/abc/def/", "/abc/def/"),
            ("abs file path with extra /", "///abc/def", "/abc/def/"),
            ("abs dir path with extra /", "///abc/def/", "/abc/def/"),
            ("dir path contains ///", "abc///def///", "/abc/def/"),
        ];

        for (name, input, expect) in cases {
            assert_eq!(normalize_root(input), expect, "{name}")
        }
    }

    #[test]
    fn test_get_basename() {
        let cases = vec![
            ("file abs path", "foo/bar/baz.txt", "baz.txt"),
            ("file rel path", "bar/baz.txt", "baz.txt"),
            ("file walk", "foo/bar/baz", "baz"),
            ("dir rel path", "bar/baz/", "baz/"),
            ("dir root", "/", "/"),
            ("dir walk", "foo/bar/baz/", "baz/"),
        ];

        for (name, input, expect) in cases {
            let actual = get_basename(input);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_get_parent() {
        let cases = vec![
            ("file abs path", "foo/bar/baz.txt", "foo/bar/"),
            ("file rel path", "bar/baz.txt", "bar/"),
            ("file walk", "foo/bar/baz", "foo/bar/"),
            ("dir rel path", "bar/baz/", "bar/"),
            ("dir root", "/", "/"),
            ("dir abs path", "/foo/bar/", "/foo/"),
            ("dir walk", "foo/bar/baz/", "foo/bar/"),
        ];

        for (name, input, expect) in cases {
            let actual = get_parent(input);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_build_abs_path() {
        let cases = vec![
            ("input abs file", "/abc/", "/", "abc/"),
            ("input dir", "/abc/", "def/", "abc/def/"),
            ("input file", "/abc/", "def", "abc/def"),
            ("input abs file with root /", "/", "/", ""),
            ("input empty with root /", "/", "", ""),
            ("input dir with root /", "/", "def/", "def/"),
            ("input file with root /", "/", "def", "def"),
        ];

        for (name, root, input, expect) in cases {
            let actual = build_abs_path(root, input);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_build_rooted_abs_path() {
        let cases = vec![
            ("input abs file", "/abc/", "/", "/abc/"),
            ("input dir", "/abc/", "def/", "/abc/def/"),
            ("input file", "/abc/", "def", "/abc/def"),
            ("input abs file with root /", "/", "/", "/"),
            ("input dir with root /", "/", "def/", "/def/"),
            ("input file with root /", "/", "def", "/def"),
        ];

        for (name, root, input, expect) in cases {
            let actual = build_rooted_abs_path(root, input);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_build_rel_path() {
        let cases = vec![
            ("input abs file", "/abc/", "/abc/def", "def"),
            ("input dir", "/abc/", "/abc/def/", "def/"),
            ("input file", "/abc/", "abc/def", "def"),
            ("input dir with root /", "/", "def/", "def/"),
            ("input file with root /", "/", "def", "def"),
        ];

        for (name, root, input, expect) in cases {
            let actual = build_rel_path(root, input);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_validate_path() {
        let cases = vec![
            ("input file with mode file", "abc", EntryMode::FILE, true),
            ("input file with mode dir", "abc", EntryMode::DIR, false),
            ("input dir with mode file", "abc/", EntryMode::FILE, false),
            ("input dir with mode dir", "abc/", EntryMode::DIR, true),
            ("root with mode dir", "/", EntryMode::DIR, true),
            (
                "input file with mode unknown",
                "abc",
                EntryMode::Unknown,
                false,
            ),
            (
                "input dir with mode unknown",
                "abc/",
                EntryMode::Unknown,
                false,
            ),
        ];

        for (name, path, mode, expect) in cases {
            let actual = validate_path(path, mode);
            assert_eq!(actual, expect, "{name}")
        }
    }

    #[test]
    fn test_build_tmp_path_of() {
        let cases = vec![
            ("a file path", "example.txt", "example.txt."),
            (
                "a file path in a directory",
                "folder/example.txt",
                "example.txt.",
            ),
        ];

        for (name, path, expect_starts_with) in cases {
            let actual = build_tmp_path_of(path);
            assert!(
                actual.starts_with(expect_starts_with),
                "{name}: got `{actual}`, but expect `{expect_starts_with}`"
            );
            assert_eq!(
                actual.len(),
                expect_starts_with.len() + 8, // See RANDOM_TMP_PATH_POSTFIX_SIZE
                "{name}: got `{actual}`, but expect `{expect_starts_with}`"
            )
        }
    }
}
