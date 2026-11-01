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

use crate::Result;
use jni::Env;
use jni::jni_sig;
use jni::jni_str;
use jni::objects::JByteArray;
use jni::objects::JMap;
use jni::objects::JObject;
use jni::objects::JString;
use jni::strings::JNIString;
use jni::sys::jlong;
use opendal::{Error, ErrorKind};
use std::collections::HashMap;
use std::ops::Bound;

pub(crate) fn usize_to_jlong(n: Option<usize>) -> jlong {
    // usize is always >= 0, so we can use -1 to identify the empty value.
    n.map_or(-1, |v| v as jlong)
}

pub(crate) fn jmap_to_hashmap(env: &mut Env, params: &JObject) -> Result<HashMap<String, String>> {
    let map = env.new_cast_local_ref::<JMap>(params)?;
    let mut iter = map.iter(env)?;

    let mut result: HashMap<String, String> = HashMap::new();
    while let Some(entry) = iter.next(env)? {
        let k = entry.key(env)?;
        let v = entry.value(env)?;
        // SAFETY: a `java.util.Map<String, String>` only yields String keys and values.
        let k = unsafe { JString::from_raw(env, k.into_raw()) };
        let v = unsafe { JString::from_raw(env, v.into_raw()) };
        result.insert(k.mutf8_chars(env)?.into(), v.mutf8_chars(env)?.into());
    }

    Ok(result)
}

pub(crate) fn hashmap_to_jmap<'a, K: AsRef<str>, V: AsRef<str>>(
    env: &mut Env<'a>,
    map: impl IntoIterator<Item = (K, V)>,
) -> Result<JObject<'a>> {
    let map_object = env.new_object(jni_str!("java/util/HashMap"), jni_sig!("()V"), &[])?;
    let jmap = env.new_cast_local_ref::<JMap>(&map_object)?;
    for (k, v) in map {
        let key = env.new_string(k.as_ref())?;
        let value = env.new_string(v.as_ref())?;
        jmap.put(env, &key, &value)?;
    }
    Ok(map_object)
}

pub(crate) fn string_to_jstring<'a>(env: &mut Env<'a>, s: Option<&str>) -> Result<JObject<'a>> {
    s.map_or_else(|| Ok(JObject::null()), |v| Ok(env.new_string(v)?.into()))
}

pub(crate) fn read_bool_field(env: &mut Env<'_>, obj: &JObject, field: &str) -> Result<bool> {
    Ok(env
        .get_field(obj, JNIString::new(field), jni_sig!("Z"))?
        .z()?)
}

pub(crate) fn read_int64_field(env: &mut Env<'_>, obj: &JObject, field: &str) -> Result<i64> {
    Ok(env
        .get_field(obj, JNIString::new(field), jni_sig!("J"))?
        .j()?)
}

pub(crate) fn read_int_field(env: &mut Env<'_>, obj: &JObject, field: &str) -> Result<i32> {
    Ok(env
        .get_field(obj, JNIString::new(field), jni_sig!("I"))?
        .i()?)
}

pub(crate) fn read_string_field(
    env: &mut Env<'_>,
    obj: &JObject,
    field: &str,
) -> Result<Option<String>> {
    let result = env
        .get_field(obj, JNIString::new(field), jni_sig!("Ljava/lang/String;"))?
        .l()?;
    if result.is_null() {
        Ok(None)
    } else {
        // SAFETY: the field is declared as `java.lang.String`.
        let result = unsafe { JString::from_raw(env, result.into_raw()) };
        Ok(Some(jstring_to_string(env, &result)?))
    }
}

pub(crate) fn read_map_field(
    env: &mut Env<'_>,
    obj: &JObject,
    field: &str,
) -> Result<Option<HashMap<String, String>>> {
    let result = env
        .get_field(obj, JNIString::new(field), jni_sig!("Ljava/util/Map;"))?
        .l()?;
    if result.is_null() {
        Ok(None)
    } else {
        Ok(Some(jmap_to_hashmap(env, &result)?))
    }
}

pub(crate) fn read_jlong_field_to_usize(
    env: &mut Env,
    options: &JObject,
    field_name: &str,
) -> Result<Option<usize>> {
    match read_int64_field(env, options, field_name)? {
        -1 => Ok(None),
        v if v > 0 => Ok(Some(v as usize)),
        v => Err(Error::new(
            ErrorKind::Unexpected,
            format!("{field_name} must be positive, instead got: {v}"),
        )
        .into()),
    }
}

pub(crate) fn read_instant_field_to_timestamp(
    env: &mut Env<'_>,
    obj: &JObject,
    field: &str,
) -> Result<Option<opendal::raw::Timestamp>> {
    let result = env
        .get_field(obj, JNIString::new(field), jni_sig!("Ljava/time/Instant;"))?
        .l()?;
    if result.is_null() {
        return Ok(None);
    }

    let epoch_second = env
        .call_method(&result, jni_str!("getEpochSecond"), jni_sig!("()J"), &[])?
        .j()?;
    let nano = env
        .call_method(&result, jni_str!("getNano"), jni_sig!("()I"), &[])?
        .i()?;
    match opendal::raw::Timestamp::new(epoch_second, nano) {
        Ok(ts) => Ok(Some(ts)),
        Err(err) => Err(Error::new(
            ErrorKind::Unexpected,
            format!("invalid timestamp: seconds={epoch_second}, nanos={nano}"),
        )
        .set_source(err)
        .into()),
    }
}

pub(crate) fn offset_length_to_range(offset: i64, length: i64) -> Result<(Bound<u64>, Bound<u64>)> {
    let offset = u64::try_from(offset)
        .map_err(|_| Error::new(ErrorKind::RangeNotSatisfied, "offset must be non-negative"))?;

    match length {
        -1 => Ok((Bound::Included(offset), Bound::Unbounded)),
        _ => match u64::try_from(length) {
            Ok(length) => match offset.checked_add(length) {
                Some(end) => Ok((Bound::Included(offset), Bound::Excluded(end))),
                None => Err(Error::new(
                    ErrorKind::RangeNotSatisfied,
                    "offset + length causes overflow",
                )
                .into()),
            },
            Err(_) => {
                Err(Error::new(ErrorKind::RangeNotSatisfied, "length must be non-negative").into())
            }
        },
    }
}

pub(crate) fn bytes_to_jbytearray<'a>(
    env: &mut Env<'a>,
    bytes: impl AsRef<[u8]>,
) -> Result<JByteArray<'a>> {
    let bytes = bytes.as_ref();
    let res = env.byte_array_from_slice(bytes)?;
    Ok(res)
}

pub(crate) fn jstring_to_string(env: &mut Env, s: &JString) -> Result<String> {
    Ok(s.mutf8_chars(env)?.into())
}
