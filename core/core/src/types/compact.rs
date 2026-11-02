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

use std::mem::MaybeUninit;
use std::sync::Arc;

const BITMAP_SIZE: usize = size_of::<u16>();
const OFFSET_SIZE: usize = size_of::<u16>();

/// Immutable storage for a closed set of optional byte fields.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct CompactValues(Option<Arc<[u8]>>);

impl CompactValues {
    #[inline]
    pub(crate) fn encode<const N: usize>(fields: &[Option<&[u8]>; N]) -> Self {
        let lengths = fields.map(|value| value.map(<[u8]>::len));
        Self::encode_with(&lengths, |index, output| {
            output.write(fields[index].expect("present field has a value"));
        })
    }

    pub(crate) fn encode_with<const N: usize>(
        lengths: &[Option<usize>; N],
        write_field: impl FnMut(usize, &mut CompactFieldWriter<'_>),
    ) -> Self {
        let layout = encoded_layout(lengths);
        let total_len = layout.2;
        if total_len == 0 {
            return Self(None);
        }

        let mut encoded = Arc::<[u8]>::new_uninit_slice(total_len);
        let output = Arc::get_mut(&mut encoded).expect("new Arc has one owner");
        write_encoded_with_layout(lengths, output, layout, write_field);

        // `write_encoded` initializes every byte in the bitmap, offsets, and payload.
        let encoded = unsafe { encoded.assume_init() };
        Self(Some(encoded))
    }

    #[inline]
    pub(crate) fn fields<const N: usize>(&self) -> [Option<&[u8]>; N] {
        assert!(N <= u16::BITS as usize);

        let mut fields = [None; N];
        let Some(encoded) = self.0.as_deref() else {
            return fields;
        };
        let present = read_present(encoded);
        let mut value_index = 0;
        let mut start = BITMAP_SIZE + present.count_ones() as usize * OFFSET_SIZE;
        for (field, value) in fields.iter_mut().enumerate() {
            if present & (1_u16 << field) == 0 {
                continue;
            }
            let end = read_offset(encoded, value_index);
            *value = Some(&encoded[start..end]);
            start = end;
            value_index += 1;
        }
        fields
    }

    #[inline]
    pub(crate) fn get_str(&self, field: usize) -> Option<&str> {
        self.get(field).map(|value| {
            // String fields only enter compact storage from `str` or `String`. Blocks decoded
            // by this module were produced by the same internal encoders.
            unsafe { std::str::from_utf8_unchecked(value) }
        })
    }

    pub(crate) fn replace(&self, field: usize, value: &[u8]) -> Self {
        assert!(field < u16::BITS as usize);
        let mut fields = self.fields::<{ u16::BITS as usize }>();
        fields[field] = Some(value);
        Self::encode(&fields)
    }

    #[inline]
    pub(crate) fn get(&self, field: usize) -> Option<&[u8]> {
        assert!(field < u16::BITS as usize);
        let encoded = self.0.as_deref()?;
        if encoded.is_empty() {
            return None;
        }
        let present = read_present(encoded);
        let bit = 1_u16 << field;
        if present & bit == 0 {
            return None;
        }

        let value_count = present.count_ones() as usize;
        let header_len = BITMAP_SIZE + value_count * OFFSET_SIZE;
        let value_index = (present & (bit - 1)).count_ones() as usize;
        let start = if value_index == 0 {
            header_len
        } else {
            read_offset(encoded, value_index - 1)
        };
        let end = read_offset(encoded, value_index);
        Some(&encoded[start..end])
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn as_encoded(&self) -> Option<&[u8]> {
        self.0.as_deref()
    }
}

fn encoded_layout(lengths: &[Option<usize>]) -> (u16, usize, usize) {
    assert!(lengths.len() <= u16::BITS as usize);

    let mut present = 0_u16;
    let mut payload_len = 0_usize;
    for (index, value) in lengths.iter().enumerate() {
        if let Some(value) = value {
            present |= 1 << index;
            payload_len = payload_len
                .checked_add(*value)
                .expect("compact value block length overflowed");
        }
    }

    if present == 0 {
        return (0, 0, 0);
    }

    let value_count = present.count_ones() as usize;
    let header_len = BITMAP_SIZE + value_count * OFFSET_SIZE;
    let total_len = header_len
        .checked_add(payload_len)
        .expect("compact value block length overflowed");
    assert!(
        total_len <= u16::MAX as usize,
        "compact value block exceeds 64 KiB"
    );

    (present, header_len, total_len)
}

fn write_encoded_with_layout(
    lengths: &[Option<usize>],
    output: &mut [MaybeUninit<u8>],
    (present, header_len, total_len): (u16, usize, usize),
    mut write_field: impl FnMut(usize, &mut CompactFieldWriter<'_>),
) {
    assert_eq!(output.len(), total_len);
    if total_len == 0 {
        return;
    }
    write_slice(output, 0, &present.to_le_bytes());

    let mut payload_offset = header_len;
    let mut value_index = 0;
    for (field, value_len) in lengths.iter().enumerate() {
        if let Some(value_len) = value_len {
            let mut writer =
                CompactFieldWriter::new(&mut output[payload_offset..payload_offset + value_len]);
            write_field(field, &mut writer);
            writer.finish();
            payload_offset += value_len;
            let end = u16::try_from(payload_offset).expect("value block length was validated");
            let offset = BITMAP_SIZE + value_index * OFFSET_SIZE;
            write_slice(output, offset, &end.to_le_bytes());
            value_index += 1;
        }
    }

    debug_assert_eq!(payload_offset, total_len);
}

fn write_slice(output: &mut [MaybeUninit<u8>], offset: usize, value: &[u8]) {
    initialize_bytes(&mut output[offset..offset + value.len()], value);
}

fn initialize_bytes(output: &mut [MaybeUninit<u8>], value: &[u8]) {
    assert_eq!(output.len(), value.len());
    for (output, value) in output.iter_mut().zip(value) {
        output.write(*value);
    }
}

pub(crate) struct CompactFieldWriter<'a> {
    output: &'a mut [MaybeUninit<u8>],
    initialized: usize,
}

impl<'a> CompactFieldWriter<'a> {
    fn new(output: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            output,
            initialized: 0,
        }
    }

    pub(crate) fn write(&mut self, value: &[u8]) {
        let end = self
            .initialized
            .checked_add(value.len())
            .expect("compact field length overflowed");
        assert!(
            end <= self.output.len(),
            "compact field wrote too many bytes"
        );
        initialize_bytes(&mut self.output[self.initialized..end], value);
        self.initialized = end;
    }

    fn finish(self) {
        assert_eq!(
            self.initialized,
            self.output.len(),
            "compact field did not initialize its declared length"
        );
    }
}

#[inline]
fn read_present(encoded: &[u8]) -> u16 {
    u16::from_le_bytes(
        encoded[..BITMAP_SIZE]
            .try_into()
            .expect("compact value block contains a bitmap"),
    )
}

#[inline]
fn read_offset(encoded: &[u8], index: usize) -> usize {
    let offset = BITMAP_SIZE + index * OFFSET_SIZE;
    u16::from_le_bytes(
        encoded[offset..offset + OFFSET_SIZE]
            .try_into()
            .expect("compact value block contains a complete offset"),
    ) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_values_roundtrip() {
        let values = CompactValues::encode(&[Some(b"first"), None, Some(b"third"), Some(b"")]);

        assert_eq!(values.get(0), Some(b"first".as_slice()));
        assert_eq!(values.get(1), None);
        assert_eq!(values.get(2), Some(b"third".as_slice()));
        assert_eq!(values.get(3), Some(b"".as_slice()));
    }

    #[test]
    fn compact_values_empty_is_default() {
        let values = CompactValues::encode(&[None, None]);
        assert_eq!(values, CompactValues::default());
    }

    #[test]
    fn compact_values_replace_present_field() {
        let values = CompactValues::encode(&[Some(b"first"), None, Some(b"third")]);
        let values = values.replace(2, b"changed");

        assert_eq!(
            values.fields::<3>(),
            [Some(b"first".as_slice()), None, Some(b"changed".as_slice())]
        );
    }

    #[test]
    fn compact_values_insert_absent_field() {
        let values = CompactValues::encode(&[Some(b"first"), None, Some(b"third")]);
        let values = values.replace(1, b"second");

        assert_eq!(
            values.fields::<3>(),
            [
                Some(b"first".as_slice()),
                Some(b"second".as_slice()),
                Some(b"third".as_slice()),
            ]
        );
    }

    #[test]
    fn compact_values_replace_preserves_high_fields() {
        let values = CompactValues::encode(&[Some(b"first"), None, None, Some(b"fourth")]);
        let values = values.replace(0, b"changed");

        assert_eq!(values.get(0), Some(b"changed".as_slice()));
        assert_eq!(values.get(3), Some(b"fourth".as_slice()));
    }

    #[test]
    fn compact_values_accept_maximum_block() {
        let value = vec![0; u16::MAX as usize - BITMAP_SIZE - OFFSET_SIZE];
        let values = CompactValues::encode(&[Some(&value)]);
        assert_eq!(values.as_encoded().unwrap().len(), u16::MAX as usize);
        assert_eq!(values.get(0), Some(value.as_slice()));
    }

    #[test]
    #[should_panic(expected = "compact value block exceeds 64 KiB")]
    fn compact_values_reject_oversized_block() {
        let value = vec![0; u16::MAX as usize - BITMAP_SIZE - OFFSET_SIZE + 1];
        CompactValues::encode(&[Some(&value)]);
    }

    #[test]
    #[should_panic(expected = "compact field did not initialize its declared length")]
    fn compact_values_reject_incomplete_encoder() {
        CompactValues::encode_with(&[Some(1)], |_, _| {});
    }
}
