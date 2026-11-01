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

//! Time related utils.

use crate::*;

use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::str::FromStr;

pub use jiff::SignedDuration;
pub use std::time::{Duration, UNIX_EPOCH};
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use std::time::{Instant, SystemTime};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use web_time::{Instant, SystemTime};

/// An instant in time represented as the number of nanoseconds since the Unix epoch.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(jiff::Timestamp);

impl FromStr for Timestamp {
    type Err = Error;

    /// Parse a timestamp by the default [`DateTimeParser`].
    ///
    /// All of them are valid time:
    ///
    /// - `2022-03-13T07:20:04Z`
    /// - `2022-03-01T08:12:34+00:00`
    /// - `2022-03-01T08:12:34.00+00:00`
    /// - `2022-07-08T02:14:07+02:00[Europe/Paris]`
    ///
    /// [`DateTimeParser`]: jiff::fmt::temporal::DateTimeParser
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse() {
            Ok(t) => Ok(Timestamp(t)),
            Err(err) => Err(Error::new(
                ErrorKind::Unexpected,
                format!("parse '{s}' into timestamp failed"),
            )
            .set_source(err)),
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Timestamp {
    /// The minimum timestamp value.
    pub const MIN: Self = Self(jiff::Timestamp::MIN);

    /// The maximum timestamp value.
    pub const MAX: Self = Self(jiff::Timestamp::MAX);

    /// Create the timestamp of now.
    pub fn now() -> Self {
        Self(jiff::Timestamp::now())
    }

    /// Format the timestamp into http date: `Sun, 06 Nov 1994 08:49:37 GMT`
    ///
    /// ## Note
    ///
    /// HTTP date is slightly different from RFC2822.
    ///
    /// - Timezone is fixed to GMT.
    /// - Day must be 2 digit.
    pub fn format_http_date(self) -> String {
        self.0.strftime("%a, %d %b %Y %T GMT").to_string()
    }

    /// Creates a new instant in time from the number of seconds elapsed since the Unix epoch.
    ///
    /// When second is negative, it corresponds to an instant in time before the Unix epoch.
    /// A smaller number corresponds to an instant in time further into the past.
    pub fn new(second: i64, nanosecond: i32) -> Result<Self, Error> {
        match jiff::Timestamp::new(second, nanosecond) {
            Ok(t) => Ok(Timestamp(t)),
            Err(err) => Err(Error::new(
                ErrorKind::Unexpected,
                format!(
                    "create timestamp from '{second}' seconds and '{nanosecond}' nanoseconds failed"
                ),
            )
            .set_source(err)),
        }
    }

    /// Creates a new instant in time from the number of milliseconds elapsed
    /// since the Unix epoch.
    ///
    /// When `millisecond` is negative, it corresponds to an instant in time
    /// before the Unix epoch. A smaller number corresponds to an instant in
    /// time further into the past.
    pub fn from_millisecond(millis: i64) -> Result<Self> {
        match jiff::Timestamp::from_millisecond(millis) {
            Ok(t) => Ok(Timestamp(t)),
            Err(err) => Err(Error::new(
                ErrorKind::Unexpected,
                format!("convert '{millis}' milliseconds into timestamp failed"),
            )
            .set_source(err)),
        }
    }

    /// Creates a new instant in time from the number of seconds elapsed since
    /// the Unix epoch.
    ///
    /// When `second` is negative, it corresponds to an instant in time before
    /// the Unix epoch. A smaller number corresponds to an instant in time
    /// further into the past.
    pub fn from_second(second: i64) -> Result<Self> {
        match jiff::Timestamp::from_second(second) {
            Ok(t) => Ok(Timestamp(t)),
            Err(err) => Err(Error::new(
                ErrorKind::Unexpected,
                format!("convert '{second}' seconds into timestamp failed"),
            )
            .set_source(err)),
        }
    }

    /// Parse a timestamp from RFC2822.
    ///
    /// All of them are valid time:
    ///
    /// - `Sat, 13 Jul 2024 15:09:59 -0400`
    /// - `Mon, 15 Aug 2022 16:50:12 GMT`
    pub fn parse_rfc2822(s: &str) -> Result<Timestamp> {
        match jiff::fmt::rfc2822::parse(s) {
            Ok(zoned) => Ok(Timestamp(zoned.timestamp())),
            Err(err) => Err(Error::new(
                ErrorKind::Unexpected,
                format!("parse '{s}' into rfc2822 failed"),
            )
            .set_source(err)),
        }
    }

    /// Convert to inner `jiff::Timestamp` for compatibility.
    ///
    /// This method is provided for accessing the underlying `jiff::Timestamp`
    /// when needed for interoperability with jiff-specific APIs.
    pub fn into_inner(self) -> jiff::Timestamp {
        self.0
    }
}

impl From<Timestamp> for jiff::Timestamp {
    fn from(t: Timestamp) -> Self {
        t.0
    }
}

impl From<jiff::Timestamp> for Timestamp {
    fn from(t: jiff::Timestamp) -> Self {
        Timestamp(t)
    }
}

impl From<Timestamp> for SystemTime {
    fn from(ts: Timestamp) -> Self {
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        {
            SystemTime::from(ts.0)
        }

        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        {
            use std::time::SystemTime as StdSystemTime;

            let t = StdSystemTime::from(ts.0);
            <web_time::SystemTime as web_time::web::SystemTimeExt>::from_std(t)
        }
    }
}

impl TryFrom<SystemTime> for Timestamp {
    type Error = Error;

    fn try_from(t: SystemTime) -> Result<Self> {
        let t = {
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            {
                t
            }

            #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
            {
                <web_time::SystemTime as web_time::web::SystemTimeExt>::to_std(t)
            }
        };

        jiff::Timestamp::try_from(t).map(Timestamp).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "input timestamp overflow").set_source(err)
        })
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Timestamp {
        let ts = self
            .0
            .checked_add(rhs)
            .expect("adding unsigned duration to timestamp overflowed");

        Timestamp(ts)
    }
}

impl AddAssign<Duration> for Timestamp {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Duration) -> Timestamp {
        let ts = self
            .0
            .checked_sub(rhs)
            .expect("subtracting unsigned duration from timestamp overflowed");

        Timestamp(ts)
    }
}

impl SubAssign<Duration> for Timestamp {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs
    }
}

/// Parse a duration encoded either as ISO-8601 (e.g. `PT5M`) or friendly (e.g. `5m`).
#[inline]
pub fn signed_to_duration(value: &str) -> Result<Duration> {
    let signed = value.parse::<SignedDuration>().map_err(|err| {
        Error::new(ErrorKind::ConfigInvalid, "failed to parse duration").set_source(err)
    })?;

    signed_duration_to_duration(signed)
}

/// Convert a jiff [`SignedDuration`] into an unsigned [`Duration`].
#[inline]
pub fn signed_duration_to_duration(value: SignedDuration) -> Result<Duration> {
    Duration::try_from(value).map_err(|err| {
        Error::new(
            ErrorKind::ConfigInvalid,
            "duration must not be negative or overflow",
        )
        .set_source(err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_time() -> Timestamp {
        Timestamp("2022-03-01T08:12:34Z".parse().unwrap())
    }

    #[test]
    fn test_format_http_date() {
        let t = test_time();
        assert_eq!("Tue, 01 Mar 2022 08:12:34 GMT", t.format_http_date())
    }

    #[test]
    fn test_parse_rfc3339() {
        let t = test_time();

        for v in [
            "2022-03-01T08:12:34Z",
            "2022-03-01T08:12:34+00:00",
            "2022-03-01T08:12:34.00+00:00",
        ] {
            assert_eq!(t, v.parse().expect("must be valid time"));
        }
    }

    #[test]
    fn test_parse_rfc2822() {
        let s = "Sat, 29 Oct 1994 19:43:31 +0000";
        let v = Timestamp::parse_rfc2822(s).unwrap();
        assert_eq!("Sat, 29 Oct 1994 19:43:31 GMT", v.format_http_date());
    }

    #[test]
    fn test_signed_duration_to_duration_rejects_negative_values() {
        assert!(signed_duration_to_duration(SignedDuration::from_secs(-1)).is_err());
    }
}
