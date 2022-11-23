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

use std::collections::VecDeque;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::ready;
use futures::Future;

use crate::object::ObjectLister;
use crate::object::ObjectPage;
use crate::object::ObjectPager;
use crate::ops::OpList;
use crate::ops::RpList;
use crate::Accessor;
use crate::Object;
use crate::ObjectEntry;
use crate::ObjectMetadata;
use crate::ObjectMode;
use crate::Result;

const WALK_BUFFER_SIZE: usize = 256;

/// TopDownWalker will walk dir in top down way:
///
/// - List current dir first
/// - Go into nested dirs one by one
///
/// Given the following file tree:
///
/// ```txt
/// .
/// ├── dir_x/
/// │   ├── dir_y/
/// │   │   ├── dir_z/
/// │   │   └── file_c
/// │   └── file_b
/// └── file_a
/// ```
///
/// TopDownWalker will output entries like:
///
/// ```txt
/// dir_x/
/// dir_x/file_a
/// dir_x/dir_y/
/// dir_x/dir_y/file_b
/// dir_x/dir_y/dir_z/
/// dir_x/dir_y/dir_z/file_c
/// ```
///
/// # Note
///
/// There is no guarantee about the order between files and dirs at the same level.
/// We only make sure the parent dirs will show up before nest dirs.
pub struct TopDownWalker {
    acc: Arc<dyn Accessor>,
    dirs: VecDeque<ObjectEntry>,
    pagers: Vec<(ObjectPager, Vec<ObjectEntry>)>,
    res: Vec<ObjectEntry>,
}

impl TopDownWalker {
    /// Create a new [`TopDownWalker`]
    pub fn new(acc: Arc<dyn Accessor>, path: &str) -> Self {
        TopDownWalker {
            acc,
            dirs: VecDeque::from([ObjectEntry::new(path, ObjectMetadata::new(ObjectMode::DIR))]),
            pagers: vec![],
            res: Vec::with_capacity(WALK_BUFFER_SIZE),
        }
    }
}

#[async_trait]
impl ObjectPage for TopDownWalker {
    async fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        todo!()
    }
}

/// BottomUpWalker will walk dir in bottom up way:
///
/// - List nested dir first
/// - Go back into parent dirs one by one
///
/// Given the following file tree:
///
/// ```txt
/// .
/// ├── dir_x/
/// │   ├── dir_y/
/// │   │   ├── dir_z/
/// │   │   └── file_c
/// │   └── file_b
/// └── file_a
/// ```
///
/// BottomUpWalker will output entries like:
///
/// ```txt
/// dir_x/dir_y/dir_z/file_c
/// dir_x/dir_y/dir_z/
/// dir_x/dir_y/file_b
/// dir_x/dir_y/
/// dir_x/file_a
/// dir_x/
/// ```
///
/// # Note
///
/// There is no guarantee about the order between files and dirs at the same level.
/// We only make sure the nested dirs will show up before parent dirs.
///
/// Especially, for storage services that can't return dirs first, BottomUpWalker
/// may output parent dirs' files before nested dirs, this is expected because files
/// always output directly while listing.
pub struct BottomUpWalker {
    acc: Arc<dyn Accessor>,
    dirs: Vec<ObjectEntry>,
    ds: Vec<ObjectPager>,
    res: Vec<ObjectEntry>,
}

impl BottomUpWalker {
    /// Create a new [`BottomUpWalker`]
    pub fn new(acc: Arc<dyn Accessor>, path: &str) -> Self {
        BottomUpWalker {
            acc,
            dirs: vec![ObjectEntry::new(path, ObjectMetadata::new(ObjectMode::DIR))],
            ds: Vec::new(),
            res: Vec::with_capacity(WALK_BUFFER_SIZE),
        }
    }
}

#[async_trait]
impl ObjectPage for BottomUpWalker {
    async fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        todo!()
    }
}

// #[cfg(test)]
// mod tests {
// use std::collections::HashSet;
// use std::env;

// use futures::TryStreamExt;
// use log::debug;

// use super::*;
// use crate::services::fs::Builder;
// use crate::Operator;

// fn get_position(vs: &[String], s: &str) -> usize {
//     vs.iter()
//         .position(|v| v == s)
//         .expect("{s} is not found in {vs}")
// }

// #[tokio::test]
// async fn test_walk_top_down() -> Result<()> {
//     let _ = env_logger::try_init();

//     let mut builder = Builder::default();
//     builder.root(&format!(
//         "{}/{}",
//         env::temp_dir().display(),
//         uuid::Uuid::new_v4()
//     ));
//     let op = Operator::new(builder.build()?);
//     let mut expected = vec![
//         "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
//     ];
//     for path in expected.iter() {
//         op.object(path).create().await?;
//     }

//     let mut set = HashSet::new();
//     let w = TopDownWalker::new(op.object("x/"));
//     let mut actual = w
//         .try_collect::<Vec<_>>()
//         .await?
//         .into_iter()
//         .map(|v| {
//             assert!(
//                 set.insert(v.path().to_string()),
//                 "duplicated value: {}",
//                 v.path()
//             );
//             v.path().to_string()
//         })
//         .collect::<Vec<_>>();

//     debug!("walk top down: {:?}", actual);

//     assert!(get_position(&actual, "x/x/x/x/") > get_position(&actual, "x/x/x/"));
//     assert!(get_position(&actual, "x/x/x/") > get_position(&actual, "x/x/"));
//     assert!(get_position(&actual, "x/x/") > get_position(&actual, "x/"));

//     expected.sort_unstable();
//     actual.sort_unstable();
//     assert_eq!(actual, expected);
//     Ok(())
// }

// #[tokio::test]
// async fn test_walk_top_down_same_level() -> Result<()> {
//     let _ = env_logger::try_init();

//     let mut builder = Builder::default();
//     builder.root(&format!(
//         "{}/{}",
//         env::temp_dir().display(),
//         uuid::Uuid::new_v4()
//     ));
//     let op = Operator::new(builder.build()?);
//     for path in ["x/x/a", "x/x/b", "x/x/c"] {
//         op.object(path).create().await?;
//     }

//     let mut set = HashSet::new();
//     let w = TopDownWalker::new(op.object(""));
//     let mut actual = w
//         .try_collect::<Vec<_>>()
//         .await?
//         .into_iter()
//         .map(|v| {
//             assert!(
//                 set.insert(v.path().to_string()),
//                 "duplicated value: {}",
//                 v.path()
//             );
//             v.path().to_string()
//         })
//         .collect::<Vec<_>>();

//     debug!("walk top down: {:?}", actual);

//     actual.sort_unstable();
//     assert_eq!(actual, {
//         let mut x = vec!["/", "x/", "x/x/", "x/x/a", "x/x/b", "x/x/c"];
//         x.sort_unstable();
//         x
//     });
//     Ok(())
// }

// #[tokio::test]
// async fn test_walk_bottom_up() -> Result<()> {
//     let _ = env_logger::try_init();

//     let mut builder = Builder::default();
//     builder.root(&format!(
//         "{}/{}",
//         env::temp_dir().display(),
//         uuid::Uuid::new_v4()
//     ));
//     let op = Operator::new(builder.build()?);
//     let mut expected = vec![
//         "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
//     ];
//     for path in expected.iter() {
//         op.object(path).create().await?;
//     }

//     let mut set = HashSet::new();
//     let w = BottomUpWalker::new(op.object("x/"));
//     let mut actual = w
//         .try_collect::<Vec<_>>()
//         .await?
//         .into_iter()
//         .map(|v| {
//             assert!(
//                 set.insert(v.path().to_string()),
//                 "duplicated value: {}",
//                 v.path()
//             );
//             v.path().to_string()
//         })
//         .collect::<Vec<_>>();

//     debug!("walk bottom up: {:?}", actual);

//     assert!(get_position(&actual, "x/x/x/x/") < get_position(&actual, "x/x/x/"));
//     assert!(get_position(&actual, "x/x/x/") < get_position(&actual, "x/x/"));
//     assert!(get_position(&actual, "x/x/") < get_position(&actual, "x/"));

//     expected.sort_unstable();
//     actual.sort_unstable();
//     assert_eq!(actual, expected);
//     Ok(())
// }
// }
