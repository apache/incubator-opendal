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

//! [`type_alias_impl_trait`](https://github.com/rust-lang/rust/issues/63063) is not stable yet.
//! So we can't write the following code:
//!
//! ```txt
//! impl Access for S3Backend {
//!     type Writer = impl oio::Write;
//! }
//! ```
//!
//! Which means we have to write the type directly like:
//!
//! ```txt
//! impl Access for OssBackend {
//!     type Writer = raw::TwoWays<
//!         oio::MultipartWriter<OssWriter>,
//!         oio::AppendWriter<OssWriter>,
//!     >;
//! }
//! ```
//!
//! This module is used to provide some enums for the above code. We should remove this module once
//! type_alias_impl_trait has been stabilized.

use crate::raw::*;
use crate::*;

/// TwoWays is used to implement traits that based on two ways.
///
/// Users can wrap two different trait types together.
pub enum TwoWays<ONE, TWO> {
    /// The first type for the [`TwoWays`].
    One(ONE),
    /// The second type for the [`TwoWays`].
    Two(TWO),
}

impl<ONE: oio::Read, TWO: oio::Read> oio::Read for TwoWays<ONE, TWO> {
    async fn read(&mut self) -> Result<Buffer> {
        match self {
            TwoWays::One(v) => v.read().await,
            TwoWays::Two(v) => v.read().await,
        }
    }
}

impl<ONE: oio::BlockingRead, TWO: oio::BlockingRead> oio::BlockingRead for TwoWays<ONE, TWO> {
    fn read(&mut self) -> Result<Buffer> {
        match self {
            Self::One(v) => v.read(),
            Self::Two(v) => v.read(),
        }
    }
}

impl<ONE: oio::Write, TWO: oio::Write> oio::Write for TwoWays<ONE, TWO> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        match self {
            Self::One(v) => v.write(bs).await,
            Self::Two(v) => v.write(bs).await,
        }
    }

    async fn close(&mut self) -> Result<Metadata> {
        match self {
            Self::One(v) => v.close().await,
            Self::Two(v) => v.close().await,
        }
    }

    async fn abort(&mut self) -> Result<()> {
        match self {
            Self::One(v) => v.abort().await,
            Self::Two(v) => v.abort().await,
        }
    }
}

impl<ONE: oio::List, TWO: oio::List> oio::List for TwoWays<ONE, TWO> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self {
            Self::One(v) => v.next().await,
            Self::Two(v) => v.next().await,
        }
    }
}

/// ThreeWays is used to implement traits that based on three ways.
///
/// Users can wrap three different trait types together.
pub enum ThreeWays<ONE, TWO, THREE> {
    /// The first type for the [`ThreeWays`].
    One(ONE),
    /// The second type for the [`ThreeWays`].
    Two(TWO),
    /// The third type for the [`ThreeWays`].
    Three(THREE),
}

impl<ONE: oio::Read, TWO: oio::Read, THREE: oio::Read> oio::Read for ThreeWays<ONE, TWO, THREE> {
    async fn read(&mut self) -> Result<Buffer> {
        match self {
            ThreeWays::One(v) => v.read().await,
            ThreeWays::Two(v) => v.read().await,
            ThreeWays::Three(v) => v.read().await,
        }
    }
}

impl<ONE: oio::BlockingRead, TWO: oio::BlockingRead, THREE: oio::BlockingRead> oio::BlockingRead
    for ThreeWays<ONE, TWO, THREE>
{
    fn read(&mut self) -> Result<Buffer> {
        match self {
            Self::One(v) => v.read(),
            Self::Two(v) => v.read(),
            Self::Three(v) => v.read(),
        }
    }
}

impl<ONE: oio::Write, TWO: oio::Write, THREE: oio::Write> oio::Write
    for ThreeWays<ONE, TWO, THREE>
{
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        match self {
            Self::One(v) => v.write(bs).await,
            Self::Two(v) => v.write(bs).await,
            Self::Three(v) => v.write(bs).await,
        }
    }

    async fn close(&mut self) -> Result<Metadata> {
        match self {
            Self::One(v) => v.close().await,
            Self::Two(v) => v.close().await,
            Self::Three(v) => v.close().await,
        }
    }

    async fn abort(&mut self) -> Result<()> {
        match self {
            Self::One(v) => v.abort().await,
            Self::Two(v) => v.abort().await,
            Self::Three(v) => v.abort().await,
        }
    }
}

/// FourWays is used to implement traits that based on four ways.
///
/// Users can wrap four different trait types together.
pub enum FourWays<ONE, TWO, THREE, FOUR> {
    /// The first type for the [`FourWays`].
    One(ONE),
    /// The second type for the [`FourWays`].
    Two(TWO),
    /// The third type for the [`FourWays`].
    Three(THREE),
    /// The fourth type for the [`FourWays`].
    Four(FOUR),
}

impl<ONE, TWO, THREE, FOUR> oio::Read for FourWays<ONE, TWO, THREE, FOUR>
where
    ONE: oio::Read,
    TWO: oio::Read,
    THREE: oio::Read,
    FOUR: oio::Read,
{
    async fn read(&mut self) -> Result<Buffer> {
        match self {
            FourWays::One(v) => v.read().await,
            FourWays::Two(v) => v.read().await,
            FourWays::Three(v) => v.read().await,
            FourWays::Four(v) => v.read().await,
        }
    }
}

impl<ONE, TWO, THREE, FOUR> oio::BlockingRead for FourWays<ONE, TWO, THREE, FOUR>
where
    ONE: oio::BlockingRead,
    TWO: oio::BlockingRead,
    THREE: oio::BlockingRead,
    FOUR: oio::BlockingRead,
{
    fn read(&mut self) -> Result<Buffer> {
        match self {
            Self::One(v) => v.read(),
            Self::Two(v) => v.read(),
            Self::Three(v) => v.read(),
            Self::Four(v) => v.read(),
        }
    }
}

impl<ONE, TWO, THREE, FOUR> oio::List for FourWays<ONE, TWO, THREE, FOUR>
where
    ONE: oio::List,
    TWO: oio::List,
    THREE: oio::List,
    FOUR: oio::List,
{
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self {
            Self::One(v) => v.next().await,
            Self::Two(v) => v.next().await,
            Self::Three(v) => v.next().await,
            Self::Four(v) => v.next().await,
        }
    }
}

impl<ONE, TWO, THREE, FOUR> oio::BlockingList for FourWays<ONE, TWO, THREE, FOUR>
where
    ONE: oio::BlockingList,
    TWO: oio::BlockingList,
    THREE: oio::BlockingList,
    FOUR: oio::BlockingList,
{
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self {
            Self::One(v) => v.next(),
            Self::Two(v) => v.next(),
            Self::Three(v) => v.next(),
            Self::Four(v) => v.next(),
        }
    }
}
