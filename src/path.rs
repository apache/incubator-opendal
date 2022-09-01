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
/// Finally, we will got path like `/path/to/root/`.
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
            .last()
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
            assert_eq!(normalize_path(input), expect, "{}", name)
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
            assert_eq!(normalize_root(input), expect, "{}", name)
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
            assert_eq!(actual, expect, "{}", name)
        }
    }

    #[test]
    fn test_build_abs_path() {
        let cases = vec![
            ("input abs file", "/abc/", "/", "abc/"),
            ("input dir", "/abc/", "def/", "abc/def/"),
            ("input file", "/abc/", "def", "abc/def"),
            ("input abs file with root /", "/", "/", ""),
            ("input dir with root /", "/", "def/", "def/"),
            ("input file with root /", "/", "def", "def"),
        ];

        for (name, root, input, expect) in cases {
            let actual = build_abs_path(root, input);
            assert_eq!(actual, expect, "{}", name)
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
            assert_eq!(actual, expect, "{}", name)
        }
    }
}
