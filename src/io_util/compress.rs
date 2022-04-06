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

//! This mod provides compress support for BytesWrite and decompress support for BytesRead.

use std::path::PathBuf;

use async_compression::futures::bufread::BrotliDecoder;
use async_compression::futures::bufread::BzDecoder;
use async_compression::futures::bufread::DeflateDecoder;
use async_compression::futures::bufread::GzipDecoder;
use async_compression::futures::bufread::LzmaDecoder;
use async_compression::futures::bufread::XzDecoder;
use async_compression::futures::bufread::ZlibDecoder;
use async_compression::futures::bufread::ZstdDecoder;
use futures::io::BufReader;

use crate::BytesRead;
use crate::BytesReader;

/// CompressAlgorithm represents all compress algorithm that OpenDAL supports.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum CompressAlgorithm {
    /// [Brotli](https://github.com/google/brotli) compression format.
    Brotli,
    /// [bzip2](http://sourceware.org/bzip2/) compression format.
    Bz2,
    /// [Deflate](https://datatracker.ietf.org/doc/html/rfc1951) Compressed Data Format.
    ///
    /// Similar to [`CompressAlgorithm::Gzip`] and [`CompressAlgorithm::Zlib`]
    Deflate,
    /// [Gzip](https://datatracker.ietf.org/doc/html/rfc1952) compress format.
    ///
    /// Similar to [`CompressAlgorithm::Deflate`] and [`CompressAlgorithm::Zlib`]
    Gzip,
    /// [LZMA](https://www.7-zip.org/sdk.html) compress format.
    Lzma,
    /// [Xz](https://tukaani.org/xz/) compress format, the successor of [`CompressAlgorithm::Lzma`].
    Xz,
    /// [Zlib](https://datatracker.ietf.org/doc/html/rfc1950) compress format.
    ///
    /// Similar to [`CompressAlgorithm::Deflate`] and [`CompressAlgorithm::Gzip`]
    Zlib,
    /// [Zstd](https://github.com/facebook/zstd) compression algorithm
    Zstd,
}

impl CompressAlgorithm {
    /// Get the file extension of this compress algorithm.
    pub fn extension(&self) -> &str {
        match self {
            CompressAlgorithm::Brotli => "br",
            CompressAlgorithm::Bz2 => "bz2",
            CompressAlgorithm::Deflate => "deflate",
            CompressAlgorithm::Gzip => "gz",
            CompressAlgorithm::Lzma => "lzma",
            CompressAlgorithm::Xz => "xz",
            CompressAlgorithm::Zlib => "zl",
            CompressAlgorithm::Zstd => "zstd",
        }
    }

    /// Create CompressAlgorithm from file extension.
    ///
    /// If the file extension is not supported, `None` will be return instead.
    pub fn from_extension(ext: &str) -> Option<CompressAlgorithm> {
        match ext {
            "br" => Some(CompressAlgorithm::Brotli),
            "bz2" => Some(CompressAlgorithm::Bz2),
            "deflate" => Some(CompressAlgorithm::Deflate),
            "gz" => Some(CompressAlgorithm::Gzip),
            "lzma" => Some(CompressAlgorithm::Lzma),
            "xz" => Some(CompressAlgorithm::Xz),
            "zl" => Some(CompressAlgorithm::Zlib),
            "zstd" => Some(CompressAlgorithm::Zstd),
            _ => None,
        }
    }

    /// Create CompressAlgorithm from file path.
    ///
    /// If the extension in file path is not supported, `None` will be return instead.
    pub fn from_path(path: &str) -> Option<CompressAlgorithm> {
        let ext = PathBuf::from(path)
            .extension()
            .map(|s| s.to_string_lossy())?
            .to_string();

        CompressAlgorithm::from_extension(&ext)
    }

    /// Wrap input reader into the corresponding reader of compress algorithm.
    pub fn into_reader<R: 'static + BytesRead>(self, r: R) -> BytesReader {
        match self {
            CompressAlgorithm::Brotli => Box::new(into_brotli_reader(r)),
            CompressAlgorithm::Bz2 => Box::new(into_bz2_reader(r)),
            CompressAlgorithm::Deflate => Box::new(into_deflate_reader(r)),
            CompressAlgorithm::Gzip => Box::new(into_gzip_reader(r)),
            CompressAlgorithm::Lzma => Box::new(into_lzma_reader(r)),
            CompressAlgorithm::Xz => Box::new(into_xz_reader(r)),
            CompressAlgorithm::Zlib => Box::new(into_zlib_reader(r)),
            CompressAlgorithm::Zstd => Box::new(into_zstd_reader(r)),
        }
    }
}

/// Wrap input reader into brotli decoder.
pub fn into_brotli_reader<R: BytesRead>(r: R) -> BrotliDecoder<BufReader<R>> {
    BrotliDecoder::new(BufReader::new(r))
}

/// Wrap input reader into bz2 decoder.
pub fn into_bz2_reader<R: BytesRead>(r: R) -> BzDecoder<BufReader<R>> {
    BzDecoder::new(BufReader::new(r))
}

/// Wrap input reader into deflate decoder.
pub fn into_deflate_reader<R: BytesRead>(r: R) -> DeflateDecoder<BufReader<R>> {
    DeflateDecoder::new(BufReader::new(r))
}

/// Wrap input reader into gzip decoder.
pub fn into_gzip_reader<R: BytesRead>(r: R) -> GzipDecoder<BufReader<R>> {
    GzipDecoder::new(BufReader::new(r))
}

/// Wrap input reader into lzma decoder.
pub fn into_lzma_reader<R: BytesRead>(r: R) -> LzmaDecoder<BufReader<R>> {
    LzmaDecoder::new(BufReader::new(r))
}

/// Wrap input reader into xz decoder.
pub fn into_xz_reader<R: BytesRead>(r: R) -> XzDecoder<BufReader<R>> {
    XzDecoder::new(BufReader::new(r))
}

/// Wrap input reader into zlib decoder.
pub fn into_zlib_reader<R: BytesRead>(r: R) -> ZlibDecoder<BufReader<R>> {
    ZlibDecoder::new(BufReader::new(r))
}

/// Wrap input reader into zstd decoder.
pub fn into_zstd_reader<R: BytesRead>(r: R) -> ZstdDecoder<BufReader<R>> {
    ZstdDecoder::new(BufReader::new(r))
}
