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

/// Ref: <https://ipld.io/specs/codecs/dag-pb/spec/>
#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct PBNode {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub data: Option<Vec<u8>>,
    #[prost(message, repeated, tag = "2")]
    pub links: Vec<PBLink>,
}

/// Ref: <https://ipld.io/specs/codecs/dag-pb/spec/>
#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct PBLink {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub hash: Option<Vec<u8>>,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(uint64, optional, tag = "3")]
    pub tsize: Option<u64>,
}

/// This type is generated by [prost_build](https://docs.rs/prost-build/latest/prost_build/) via proto file `https://github.com/ipfs/go-unixfs/raw/master/pb/unixfs.proto`.
//
/// No modification has been and will be made from OpenDAL.
#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Data {
    #[prost(enumeration = "data::DataType", required, tag = "1")]
    pub r#type: i32,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub data: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "3")]
    pub filesize: Option<u64>,
    #[prost(uint64, repeated, packed = "false", tag = "4")]
    pub blocksizes: Vec<u64>,
    #[prost(uint64, optional, tag = "5")]
    pub hash_type: Option<u64>,
    #[prost(uint64, optional, tag = "6")]
    pub fanout: Option<u64>,
}

/// Nested message and enum types in `Data`.
pub mod data {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
    #[repr(i32)]
    pub enum DataType {
        Raw = 0,
        Directory = 1,
        File = 2,
        Metadata = 3,
        Symlink = 4,
        HamtShard = 5,
    }
    impl DataType {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                DataType::Raw => "Raw",
                DataType::Directory => "Directory",
                DataType::File => "File",
                DataType::Metadata => "Metadata",
                DataType::Symlink => "Symlink",
                DataType::HamtShard => "HAMTShard",
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct Metadata {
    #[prost(string, optional, tag = "1")]
    pub mime_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use data::DataType;
    use prost::Message;

    use super::*;

    /// Content is generated from `https://ipfs.io/ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ`
    /// with `accept: application/vnd.ipld.raw`
    #[test]
    fn test_message() {
        let bs: Vec<u8> = vec![
            0o022, 0o062, 0o012, 0o042, 0o022, 0o040, 0o220, 0o124, 0o225, 0o170, 0o152, 0o036,
            0o040, 0o072, 0o032, 0o053, 0o143, 0o274, 0o044, 0o366, 0o215, 0o306, 0o177, 0o041,
            0o260, 0o310, 0o231, 0o271, 0o142, 0o163, 0o006, 0o150, 0o054, 0o315, 0o227, 0o375,
            0o347, 0o143, 0o022, 0o012, 0o156, 0o157, 0o162, 0o155, 0o141, 0o154, 0o137, 0o144,
            0o151, 0o162, 0o030, 0o074, 0o022, 0o065, 0o012, 0o042, 0o022, 0o040, 0o337, 0o200,
            0o021, 0o267, 0o364, 0o242, 0o030, 0o145, 0o257, 0o150, 0o205, 0o335, 0o100, 0o173,
            0o222, 0o053, 0o074, 0o051, 0o041, 0o020, 0o203, 0o265, 0o116, 0o223, 0o111, 0o145,
            0o040, 0o114, 0o350, 0o143, 0o354, 0o002, 0o022, 0o013, 0o156, 0o157, 0o162, 0o155,
            0o141, 0o154, 0o137, 0o146, 0o151, 0o154, 0o145, 0o030, 0o216, 0o200, 0o020, 0o022,
            0o064, 0o012, 0o042, 0o022, 0o040, 0o051, 0o305, 0o176, 0o211, 0o142, 0o044, 0o020,
            0o267, 0o344, 0o172, 0o166, 0o374, 0o043, 0o010, 0o354, 0o047, 0o061, 0o031, 0o021,
            0o121, 0o367, 0o014, 0o003, 0o002, 0o343, 0o032, 0o250, 0o353, 0o316, 0o263, 0o224,
            0o142, 0o022, 0o012, 0o157, 0o156, 0o164, 0o151, 0o155, 0o145, 0o056, 0o143, 0o163,
            0o166, 0o030, 0o215, 0o307, 0o005, 0o022, 0o067, 0o012, 0o042, 0o022, 0o040, 0o215,
            0o225, 0o000, 0o035, 0o077, 0o302, 0o322, 0o271, 0o150, 0o077, 0o364, 0o202, 0o062,
            0o144, 0o104, 0o074, 0o327, 0o111, 0o131, 0o233, 0o176, 0o144, 0o033, 0o076, 0o266,
            0o144, 0o203, 0o256, 0o107, 0o257, 0o161, 0o112, 0o022, 0o016, 0o157, 0o156, 0o164,
            0o151, 0o155, 0o145, 0o056, 0o143, 0o163, 0o166, 0o056, 0o142, 0o172, 0o062, 0o030,
            0o206, 0o065, 0o022, 0o066, 0o012, 0o042, 0o022, 0o040, 0o205, 0o065, 0o360, 0o241,
            0o222, 0o377, 0o267, 0o213, 0o334, 0o057, 0o060, 0o130, 0o230, 0o154, 0o213, 0o260,
            0o123, 0o143, 0o011, 0o055, 0o365, 0o103, 0o002, 0o332, 0o213, 0o150, 0o275, 0o164,
            0o162, 0o223, 0o350, 0o117, 0o022, 0o015, 0o157, 0o156, 0o164, 0o151, 0o155, 0o145,
            0o056, 0o143, 0o163, 0o166, 0o056, 0o147, 0o172, 0o030, 0o217, 0o101, 0o022, 0o067,
            0o012, 0o042, 0o022, 0o040, 0o202, 0o041, 0o276, 0o325, 0o105, 0o143, 0o237, 0o357,
            0o121, 0o152, 0o300, 0o112, 0o067, 0o205, 0o022, 0o226, 0o021, 0o015, 0o302, 0o061,
            0o135, 0o225, 0o320, 0o123, 0o030, 0o101, 0o007, 0o367, 0o157, 0o273, 0o154, 0o306,
            0o022, 0o016, 0o157, 0o156, 0o164, 0o151, 0o155, 0o145, 0o056, 0o143, 0o163, 0o166,
            0o056, 0o172, 0o163, 0o164, 0o030, 0o251, 0o103, 0o022, 0o111, 0o012, 0o042, 0o022,
            0o040, 0o220, 0o124, 0o225, 0o170, 0o152, 0o036, 0o040, 0o072, 0o032, 0o053, 0o143,
            0o274, 0o044, 0o366, 0o215, 0o306, 0o177, 0o041, 0o260, 0o310, 0o231, 0o271, 0o142,
            0o163, 0o006, 0o150, 0o054, 0o315, 0o227, 0o375, 0o347, 0o143, 0o022, 0o041, 0o163,
            0o160, 0o145, 0o143, 0o151, 0o141, 0o154, 0o137, 0o144, 0o151, 0o162, 0o040, 0o040,
            0o041, 0o100, 0o043, 0o044, 0o045, 0o136, 0o046, 0o052, 0o050, 0o051, 0o137, 0o053,
            0o055, 0o075, 0o073, 0o047, 0o076, 0o074, 0o054, 0o077, 0o030, 0o074, 0o022, 0o114,
            0o012, 0o042, 0o022, 0o040, 0o337, 0o200, 0o021, 0o267, 0o364, 0o242, 0o030, 0o145,
            0o257, 0o150, 0o205, 0o335, 0o100, 0o173, 0o222, 0o053, 0o074, 0o051, 0o041, 0o020,
            0o203, 0o265, 0o116, 0o223, 0o111, 0o145, 0o040, 0o114, 0o350, 0o143, 0o354, 0o002,
            0o022, 0o042, 0o163, 0o160, 0o145, 0o143, 0o151, 0o141, 0o154, 0o137, 0o146, 0o151,
            0o154, 0o145, 0o040, 0o040, 0o041, 0o100, 0o043, 0o044, 0o045, 0o136, 0o046, 0o052,
            0o050, 0o051, 0o137, 0o053, 0o055, 0o075, 0o073, 0o047, 0o076, 0o074, 0o054, 0o077,
            0o030, 0o216, 0o200, 0o020, 0o012, 0o002, 0o010, 0o001,
        ];

        let data = PBNode::decode(Bytes::from(bs)).expect("decode must succeed");
        if let Some(bs) = data.data.clone() {
            let d = Data::decode(Bytes::from(bs)).expect("decode must succeed");
            assert_eq!(d.r#type, DataType::Directory as i32);
        }

        assert_eq!(data.links.len(), 8);
        assert_eq!(data.links[0].name.as_ref().unwrap(), "normal_dir");
        assert_eq!(data.links[0].tsize.unwrap(), 60);

        assert_eq!(data.links[1].name.as_ref().unwrap(), "normal_file");
        assert_eq!(data.links[1].tsize.unwrap(), 262158);
    }
}
