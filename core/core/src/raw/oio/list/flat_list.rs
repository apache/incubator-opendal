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

use std::sync::Arc;

use crate::raw::*;
use crate::*;

/// FlatLister will walk dir in bottom up way:
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
/// ToFlatLister will output entries like:
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
/// Especially, for storage services that can't return dirs first, ToFlatLister
/// may output parent dirs' files before nested dirs, this is expected because files
/// always output directly while listing.
pub struct FlatLister<S: Service> {
    service: Arc<S>,
    ctx: OperationContext,

    next_dir: Option<oio::Entry>,
    active_lister: Vec<(Option<oio::Entry>, S::Lister)>,
}

/// # Safety
///
/// wasm32 is a special target that we only have one event-loop for this FlatLister.
unsafe impl<S: Service> Send for FlatLister<S> {}
/// # Safety
///
/// We will only take `&mut Self` reference for FsLister.
unsafe impl<S: Service> Sync for FlatLister<S> {}

impl<S: Service> FlatLister<S> {
    /// Create a new flat lister
    pub fn new(service: Arc<S>, ctx: OperationContext, path: &str) -> FlatLister<S> {
        FlatLister {
            service,
            ctx,
            next_dir: Some(oio::Entry::new(path, MetadataBuilder::dir().build())),
            active_lister: vec![],
        }
    }
}

impl<S: Service> oio::List for FlatLister<S> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            if let Some(de) = self.next_dir.take() {
                let mut l = match self
                    .service
                    .as_ref()
                    .list(&self.ctx, de.path(), OpList::new())
                {
                    Ok(v) => v,
                    Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                        // Skip directories that we don't have permission to access
                        // and continue with the rest of the listing.
                        log::warn!(
                            "FlatLister skipping directory due to permission denied: {}",
                            de.path()
                        );
                        continue;
                    }
                    Err(e) if e.kind() == ErrorKind::NotFound => {
                        // Skip directories that are deleted while listing.
                        log::warn!(
                            "FlatLister skipping directory due to not found during listing: {}",
                            de.path()
                        );
                        continue;
                    }
                    Err(e) => return Err(e),
                };
                let first = loop {
                    match l.next().await {
                        Ok(v) => break v,
                        Err(e) if e.kind() == ErrorKind::NotFound => {
                            // Skip entries that are deleted during listing.
                            log::warn!(
                                "FlatLister skipping entry due to not found during listing: {}",
                                de.path()
                            );
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                };
                if let Some(v) = first {
                    self.active_lister.push((Some(de.clone()), l));

                    if v.mode().is_dir() {
                        // should not loop itself again
                        if v.path() != de.path() {
                            self.next_dir = Some(v);
                            continue;
                        }
                    } else {
                        return Ok(Some(v));
                    }
                }
            }

            if matches!(self.active_lister.last(), Some((None, _))) {
                let _ = self.active_lister.pop();
                continue;
            }

            let (de, lister) = match self.active_lister.last_mut() {
                Some((de, lister)) => (de, lister),
                None => return Ok(None),
            };

            match lister.next().await {
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    let path = de.as_ref().map(|entry| entry.path()).unwrap_or("<unknown>");
                    log::warn!(
                        "FlatLister skipping entry due to not found during recursive listing: {}",
                        path
                    );
                    continue;
                }
                Err(e) => return Err(e),
                Ok(Some(v)) if v.mode().is_dir() => {
                    // should not loop itself again
                    if v.path()
                        != de
                            .as_ref()
                            .expect("de must be present before listing")
                            .path()
                    {
                        self.next_dir = Some(v);
                        continue;
                    }
                }
                Ok(Some(v)) => return Ok(Some(v)),
                Ok(None) => match de.take() {
                    Some(de) => {
                        return Ok(Some(de));
                    }
                    None => {
                        let _ = self.active_lister.pop();
                        continue;
                    }
                },
            }
        }
    }
}
