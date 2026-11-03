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

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyType;
use pyo3_async_runtimes::tokio::future_into_py;

use crate::*;

fn build_operator(scheme: &str, map: HashMap<String, String>) -> PyResult<ocore::Operator> {
    let op = ocore::Operator::via_iter(scheme, map).map_err(format_pyerr)?;
    Ok(op)
}

fn build_blocking_operator(
    scheme: &str,
    map: HashMap<String, String>,
) -> PyResult<ocore::blocking::Operator> {
    let op = ocore::Operator::via_iter(scheme, map).map_err(format_pyerr)?;

    let runtime = pyo3_async_runtimes::tokio::get_runtime();
    let _guard = runtime.enter();
    let op = ocore::blocking::Operator::new(op).map_err(format_pyerr)?;
    Ok(op)
}

fn build_operator_from_uri(uri: &str, map: HashMap<String, String>) -> PyResult<ocore::Operator> {
    let op = ocore::Operator::from_uri((uri, map)).map_err(format_pyerr)?;
    Ok(op)
}

fn build_blocking_operator_from_uri(
    uri: &str,
    map: HashMap<String, String>,
) -> PyResult<ocore::blocking::Operator> {
    let op = build_operator_from_uri(uri, map)?;

    let runtime = pyo3_async_runtimes::tokio::get_runtime();
    let _guard = runtime.enter();
    let op = ocore::blocking::Operator::new(op).map_err(format_pyerr)?;
    Ok(op)
}

fn normalize_scheme(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('_', "-")
}

fn extract_delete_paths(paths: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    // A scalar path must not become an iterable of characters or components.
    if paths.is_instance_of::<PyString>()
        || paths.is_instance_of::<PyBytes>()
        || paths.hasattr("__fspath__")?
    {
        return Err(PyTypeError::new_err(
            "delete_many requires an iterable of paths, not a single path",
        ));
    }

    paths
        .try_iter()?
        .map(|entry| {
            let path = entry?.extract::<PathBuf>()?;
            Ok(path.to_string_lossy().into_owned())
        })
        .collect()
}

/// Convert a config value into the string form core's config deserializer
/// consumes.
///
/// Accepts the native types the `opendal.config` TypedDicts declare: `str`,
/// `bool`, `int`, `os.PathLike`, and `list`/`tuple` of those (`,`-joined, as
/// core parses `Vec`). A nested `dict` has no flat-map form and is rejected.
fn config_value_to_string(value: &Bound<PyAny>) -> PyResult<String> {
    // `str` before the list branch (a `str` is also a sequence); `bool` before
    // `int` (a Python `bool` also extracts as `int`); `dict` before the list
    // branch so a map is rejected rather than read as its keys.
    if let Ok(s) = value.extract::<String>() {
        Ok(s)
    } else if let Ok(b) = value.extract::<bool>() {
        Ok(if b { "true" } else { "false" }.to_string())
    } else if let Ok(i) = value.extract::<i128>() {
        Ok(i.to_string())
    } else if value.cast::<PyDict>().is_ok() {
        Err(Unsupported::new_err(
            "a map-valued config field cannot be built via from_config; leave it unset",
        ))
    } else if let Ok(items) = value.extract::<Vec<Bound<PyAny>>>() {
        let parts = items
            .iter()
            .map(config_value_to_string)
            .collect::<PyResult<Vec<_>>>()?;
        Ok(parts.join(","))
    } else if let Ok(path) = value.extract::<PathBuf>() {
        Ok(path.to_string_lossy().into_owned())
    } else {
        Err(Unsupported::new_err(
            "unsupported config value type; pass a str, bool, int, os.PathLike, or list of those",
        ))
    }
}

/// Extract `(scheme, config_map)` from a typed service config dict.
///
/// A config is a plain `dict` (an `opendal.config.ServiceConfig`, e.g.
/// `S3Config`) whose `scheme` key selects the service; every other pair becomes
/// a config option, converted via [`config_value_to_string`].
fn extract_typed_config(config: &Bound<PyAny>) -> PyResult<(String, HashMap<String, String>)> {
    let dict = config.cast::<PyDict>().map_err(|_| {
        Unsupported::new_err(
            "from_config expects an opendal.config.ServiceConfig \
             (a dict with a 'scheme' key, e.g. S3Config(scheme=\"s3\", ...))",
        )
    })?;

    let scheme = dict
        .get_item("scheme")?
        .ok_or_else(|| Unsupported::new_err("config is missing the required 'scheme' key"))?
        .extract::<String>()?;

    let mut map = HashMap::with_capacity(dict.len());
    for (k, v) in dict.iter() {
        let key = k.extract::<String>()?;
        if key != "scheme" {
            let value = config_value_to_string(&v)?;
            map.insert(key, value);
        }
    }

    Ok((scheme, map))
}

/// Rebuild a blocking [`Operator`] while unpickling.
///
/// Routes through `from_uri`, not the scheme-based `__new__`, whose scheme
/// normalization would corrupt a URI held in `__scheme`. Bare schemes work too:
/// the core resolves both through the same path.
#[pyfunction]
pub fn _reconstruct_operator(scheme: &str, map: HashMap<String, String>) -> PyResult<Operator> {
    Ok(Operator {
        core: build_blocking_operator_from_uri(scheme, map.clone())?,
        __scheme: scheme.to_string(),
        __map: map,
    })
}

/// Rebuild an [`AsyncOperator`] while unpickling.
///
/// See [`_reconstruct_operator`] for why a dedicated reconstructor is used.
#[pyfunction]
pub fn _reconstruct_async_operator(
    scheme: &str,
    map: HashMap<String, String>,
) -> PyResult<AsyncOperator> {
    Ok(AsyncOperator {
        core: build_operator_from_uri(scheme, map.clone())?,
        __scheme: scheme.to_string(),
        __map: map,
    })
}

/// The blocking equivalent of `AsyncOperator`.
///
/// `Operator` is the entry point for all blocking APIs.
///
/// See also
/// --------
/// AsyncOperator
#[pyclass(module = "opendal.operator")]
pub struct Operator {
    core: ocore::blocking::Operator,
    __scheme: String,
    __map: HashMap<String, String>,
}
#[pymethods]
impl Operator {
    /// Create a new blocking `Operator`.
    ///
    /// Parameters
    /// ----------
    /// scheme : str | Scheme
    ///     The scheme of the service.
    /// **kwargs : dict
    ///     The options for the service. Path-like values are converted to strings.
    ///
    /// Returns
    /// -------
    /// Operator
    ///     The new operator.
    #[new]
    #[pyo3(signature = (scheme: "str | Scheme", *, **kwargs: "str | PathLike[str]"))]
    pub fn new(
        scheme: Bound<PyAny>,
        kwargs: Option<HashMap<String, Bound<PyAny>>>,
    ) -> PyResult<Self> {
        let scheme = if let Ok(scheme_str) = scheme.extract::<&str>() {
            scheme_str.to_string()
        } else if let Ok(py_scheme) = scheme.extract::<Scheme>() {
            String::from(py_scheme)
        } else {
            return Err(Unsupported::new_err(
                "Invalid type for scheme, expected str or Scheme",
            ));
        };
        let scheme = normalize_scheme(&scheme);
        let map = kwargs
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| {
                let value = value.extract::<String>().or_else(|_| {
                    value
                        .py()
                        .import("os")?
                        .getattr("fspath")?
                        .call1((&value,))?
                        .extract::<String>()
                })?;
                Ok((key, value))
            })
            .collect::<PyResult<HashMap<_, _>>>()?;

        Ok(Operator {
            core: build_blocking_operator(&scheme, map.clone())?,
            __scheme: scheme,
            __map: map,
        })
    }

    /// Create a new blocking `Operator` from a URI string.
    ///
    /// The URI encodes the scheme and configuration in a single string, e.g.
    /// ``memory://`` or ``s3://bucket/path?region=us-east-1``. The scheme must
    /// belong to a service enabled in this build. Encode service options as
    /// query parameters; use ``urllib.parse.urlencode`` when building the URI
    /// dynamically.
    ///
    /// Parameters
    /// ----------
    /// uri : str
    ///     The URI of the service, including any options as query parameters.
    /// **kwargs : dict
    ///     Overrides for URI options. Prefer the URI query string.
    ///
    /// Returns
    /// -------
    /// Operator
    ///     The new operator.
    ///
    /// Examples
    /// --------
    /// ```python
    /// from urllib.parse import urlencode
    /// import opendal
    ///
    /// op = opendal.Operator.from_uri("memory://")
    /// query = urlencode({"region": "us-east-1"})
    /// op = opendal.Operator.from_uri(f"s3://bucket/path?{query}")
    /// ```
    #[classmethod]
    #[pyo3(signature = (uri, **kwargs: "str"))]
    pub fn from_uri(
        _cls: &Bound<PyType>,
        uri: &str,
        kwargs: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        let map = kwargs.unwrap_or_default();

        Ok(Operator {
            core: build_blocking_operator_from_uri(uri, map.clone())?,
            __scheme: uri.to_string(),
            __map: map,
        })
    }

    /// Create a new blocking `Operator` from a typed service config.
    ///
    /// The config is an ``opendal.config.ServiceConfig`` (e.g. ``S3Config``); its
    /// ``scheme`` key selects the service, so a static type checker rejects a
    /// wrong scheme, a missing required key, an unknown key, and a wrong value
    /// type. Non-string values (``bool``, ``int``, ``os.PathLike``, ``list``)
    /// are converted to the string form core consumes.
    ///
    /// Parameters
    /// ----------
    /// config : ServiceConfig
    ///     A service configuration such as ``opendal.config.S3Config``.
    ///
    /// Returns
    /// -------
    /// Operator
    ///     The new operator.
    ///
    /// Examples
    /// --------
    /// ```python
    /// import opendal
    /// from opendal.config import S3Config
    ///
    /// op = opendal.Operator.from_config(S3Config(scheme="s3", bucket="my-bucket"))
    /// ```
    #[classmethod]
    #[pyo3(signature = (config: "ServiceConfig"))]
    pub fn from_config(_cls: &Bound<PyType>, config: &Bound<PyAny>) -> PyResult<Self> {
        let (scheme, map) = extract_typed_config(config)?;
        let scheme = normalize_scheme(&scheme);

        Ok(Operator {
            core: build_blocking_operator(&scheme, map.clone())?,
            __scheme: scheme,
            __map: map,
        })
    }

    /// Add a new layer to this operator.
    ///
    /// Parameters
    /// ----------
    /// layer : Layer
    ///     The layer to add.
    ///
    /// Returns
    /// -------
    /// Operator
    ///     A new operator with the layer added.
    pub fn layer(&self, layer: &layers::Layer) -> PyResult<Self> {
        let op = layer.0.layer(self.core.clone().into());

        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        let _guard = runtime.enter();
        let op = ocore::blocking::Operator::new(op).map_err(format_pyerr)?;
        Ok(Self {
            core: op,
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    /// Open a file-like object for the given path.
    ///
    /// The returning file-like object is a context manager.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// mode : str
    ///     The mode to open the file in. Only "rb" and "wb" are supported.
    /// **kwargs
    ///     Additional options for the underlying reader or writer.
    ///
    /// Returns
    /// -------
    /// File
    ///     A file-like object.
    #[pyo3(signature = (path, mode, *, **kwargs: "Unpack[OpenKwargs]"))]
    pub fn open(
        &self,
        py: Python<'_>,
        path: PathBuf,
        mode: String,
        kwargs: Option<&Bound<PyDict>>,
    ) -> PyResult<File> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();

        let reader_opts = kwargs
            .map(|v| v.extract::<ReadOptions>())
            .transpose()?
            .unwrap_or_default();

        let writer_opts = kwargs
            .map(|v| v.extract::<WriteOptions>())
            .transpose()?
            .unwrap_or_default();

        if mode == "rb" {
            let range = reader_opts.make_range();
            let r = py
                .detach(move || {
                    let reader = this.reader_options(&path, reader_opts.into())?;
                    reader.into_std_read(range.to_range())
                })
                .map_err(format_pyerr)?;
            Ok(File::new_reader(r))
        } else if mode == "wb" {
            let mut writer_opts = writer_opts;
            // Keep small writes coalesced without the std::io adapter when
            // neither the caller nor the service supplies a chunk size.
            if writer_opts.chunk.is_none()
                && this.info().capability().write_multi_min_size.is_none()
            {
                writer_opts.chunk = Some(256 * 1024);
            }
            let writer = py
                .detach(move || {
                    let op: ocore::Operator = this.into();
                    pyo3_async_runtimes::tokio::get_runtime()
                        .handle()
                        .block_on(op.writer_options(&path, writer_opts.into()))
                })
                .map_err(format_pyerr)?;
            Ok(File::new_writer(writer))
        } else {
            Err(Unsupported::new_err(format!(
                "OpenDAL doesn't support mode: {mode}"
            )))
        }
    }

    /// Read the entire contents of a file at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// version : str, optional
    ///     The version of the file.
    /// concurrent : int, optional
    ///     The number of concurrent readers.
    /// chunk : int, optional
    ///     The size of each chunk.
    /// gap : int, optional
    ///     The gap between each chunk.
    /// offset : int, optional
    ///     The offset of the file.
    /// prefetch : int, optional
    ///     The number of bytes to prefetch.
    /// size : int, optional
    ///     The size of the file.
    /// if_match : str, optional
    ///     The ETag of the file.
    /// if_none_match : str, optional
    ///     The ETag of the file.
    /// if_modified_since : str, optional
    ///     The last modified time of the file.
    /// if_unmodified_since : str, optional
    ///     The last modified time of the file.
    /// content_type : str, optional
    ///     The content type of the file.
    /// cache_control : str, optional
    ///     The cache control of the file.
    /// content_disposition : str, optional
    ///     The content disposition of the file.
    ///
    /// Returns
    /// -------
    /// bytes
    ///     The contents of the file as bytes.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        version=None,
        concurrent=None,
        chunk=None,
        gap=None,
        offset=None,
        prefetch=None,
        size=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None) -> "bytes")]
    pub fn read<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        version: Option<String>,
        concurrent: Option<usize>,
        chunk: Option<usize>,
        gap: Option<usize>,
        offset: Option<usize>,
        prefetch: Option<usize>,
        size: Option<usize>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let path = path.to_string_lossy().to_string();
        let opts = ReadOptions {
            version,
            concurrent,
            chunk,
            gap,
            offset,
            prefetch,
            size,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
        };
        let this = self.core.clone();
        let buffer = py
            .detach(move || this.read_options(&path, opts.into()))
            .map_err(format_pyerr)?;

        Ok(buffer_into_py_bytes(py, buffer)?.into_any())
    }

    /// Write bytes to a file at the given path.
    ///
    /// This function will create a file if it does not exist, and will
    /// overwrite its contents if it does.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// bs : bytes
    ///     The contents to write to the file.
    /// append : bool, optional
    ///     Whether to append to the file instead of overwriting it.
    /// chunk : int, optional
    ///     The chunk size to use when writing the file.
    /// concurrent : int, optional
    ///     The number of concurrent requests to make when writing the file.
    /// cache_control : str, optional
    ///     The cache control header to set on the file.
    /// content_type : str, optional
    ///     The content type header to set on the file.
    /// content_disposition : str, optional
    ///     The content disposition header to set on the file.
    /// content_encoding : str, optional
    ///     The content encoding header to set on the file.
    /// if_match : str, optional
    ///     The ETag to match when writing the file.
    /// if_none_match : str, optional
    ///     The ETag to not match when writing the file.
    /// if_not_exists : bool, optional
    ///     Whether to fail if the file already exists.
    /// user_metadata : dict, optional
    ///     The user metadata to set on the file.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, bs: "bytes", *,
        append= None,
        chunk = None,
        concurrent = None,
        cache_control = None,
        content_type = None,
        content_disposition = None,
        content_encoding = None,
        if_match = None,
        if_none_match = None,
        if_not_exists = None,
        user_metadata = None))]
    pub fn write(
        &self,
        py: Python<'_>,
        path: PathBuf,
        bs: &Bound<PyAny>,
        append: Option<bool>,
        chunk: Option<usize>,
        concurrent: Option<usize>,
        cache_control: Option<String>,
        content_type: Option<String>,
        content_disposition: Option<String>,
        content_encoding: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_not_exists: Option<bool>,
        user_metadata: Option<HashMap<String, String>>,
    ) -> PyResult<()> {
        let path = path.to_string_lossy().to_string();
        let bs = py_bytes_like_into_buffer(bs)?;
        let opts = WriteOptions {
            append,
            chunk,
            concurrent,
            cache_control,
            content_type,
            content_disposition,
            content_encoding,
            if_match,
            if_none_match,
            if_not_exists,
            user_metadata,
        };

        let this = self.core.clone();
        py.detach(move || this.write_options(&path, bs, opts.into()))
            .map(|_| ())
            .map_err(format_pyerr)
    }

    /// Get the metadata of a file at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// version : str, optional
    ///     The version of the file.
    /// if_match : str, optional
    ///     The ETag of the file.
    /// if_none_match : str, optional
    ///     The ETag of the file.
    /// if_modified_since : datetime, optional
    ///     The last modified time of the file.
    /// if_unmodified_since : datetime, optional
    ///     The last modified time of the file.
    /// content_type : str, optional
    ///     The content type of the file.
    /// cache_control : str, optional
    ///     The cache control of the file.
    /// content_disposition : str, optional
    ///     The content disposition of the file.
    ///
    /// Returns
    /// -------
    /// Metadata
    ///     The metadata of the file.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        version=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None))]
    pub fn stat(
        &self,
        py: Python<'_>,
        path: PathBuf,
        version: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Metadata> {
        let path = path.to_string_lossy().to_string();
        let opts = StatOptions {
            version,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
        };
        let this = self.core.clone();
        py.detach(move || this.stat_options(&path, opts.into()))
            .map_err(format_pyerr)
            .map(Metadata::new)
    }

    /// Copy a file from one path to another.
    ///
    /// Parameters
    /// ----------
    /// source : str
    ///     The path to the source file.
    /// target : str
    ///     The path to the target file.
    pub fn copy(&self, py: Python<'_>, source: PathBuf, target: PathBuf) -> PyResult<()> {
        let source = source.to_string_lossy().to_string();
        let target = target.to_string_lossy().to_string();
        let this = self.core.clone();
        py.detach(move || this.copy(&source, &target))
            .map(|_| ())
            .map_err(format_pyerr)
    }

    /// Rename (move) a file from one path to another.
    ///
    /// Parameters
    /// ----------
    /// source : str
    ///     The path to the source file.
    /// target : str
    ///     The path to the target file.
    pub fn rename(&self, py: Python<'_>, source: PathBuf, target: PathBuf) -> PyResult<()> {
        let source = source.to_string_lossy().to_string();
        let target = target.to_string_lossy().to_string();
        let this = self.core.clone();
        py.detach(move || this.rename(&source, &target))
            .map_err(format_pyerr)
    }

    /// Recursively remove all files and directories at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to remove.
    pub fn remove_all(&self, py: Python<'_>, path: PathBuf) -> PyResult<()> {
        use ocore::options::DeleteOptions;
        let path = path.to_string_lossy().to_string();
        let this = self.core.clone();
        py.detach(move || {
            this.delete_options(
                &path,
                DeleteOptions {
                    recursive: true,
                    ..Default::default()
                },
            )
        })
        .map_err(format_pyerr)
    }

    /// Create a directory at the given path.
    ///
    /// Notes
    /// -----
    /// To indicate that a path is a directory, it must end with a `/`.
    /// This operation is always recursive, like `mkdir -p`.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    pub fn create_dir(&self, py: Python<'_>, path: PathBuf) -> PyResult<()> {
        let path = path.to_string_lossy().to_string();
        let this = self.core.clone();
        py.detach(move || this.create_dir(&path))
            .map_err(format_pyerr)
    }

    /// Delete a file at the given path.
    ///
    /// Notes
    /// -----
    /// This operation will not return an error if the path does not exist.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// version : str, optional
    ///     The version of the file to delete. Only supported on version-aware backends.
    /// recursive : bool, optional
    ///     If True, delete the path recursively.
    ///     Only supported on backends that support recursive delete.
    /// if_match : str, optional
    ///     If set, only delete when the existing object's ETag matches.
    #[pyo3(signature = (path, *, version=None, recursive=None, if_match=None))]
    pub fn delete(
        &self,
        py: Python<'_>,
        path: PathBuf,
        version: Option<String>,
        recursive: Option<bool>,
        if_match: Option<String>,
    ) -> PyResult<()> {
        let path = path.to_string_lossy().to_string();
        let this = self.core.clone();
        if version.is_some() || recursive.is_some() || if_match.is_some() {
            let opts = ocore::options::DeleteOptions {
                version,
                recursive: recursive.unwrap_or(false),
                if_match,
                ..Default::default()
            };
            py.detach(move || this.delete_options(&path, opts))
                .map_err(format_pyerr)
        } else {
            py.detach(move || this.delete(&path)).map_err(format_pyerr)
        }
    }

    /// Delete an explicit collection of paths with default delete options.
    ///
    /// Notes
    /// -----
    /// This method collects the iterable and converts its Python path objects
    /// before deletion starts, using memory proportional to the number of paths.
    /// Empty collections do nothing. Paths that do not exist are ignored.
    /// Storage or path-validation errors can leave some paths deleted; this
    /// operation is not atomic and does not guarantee a single remote request.
    /// Use `remove_all` to recursively delete a path prefix.
    ///
    /// Parameters
    /// ----------
    /// paths : Iterable[str | PathLike[str]]
    ///     The paths to delete. A single path is not accepted.
    #[pyo3(signature = (paths: "collections.abc.Iterable[str | PathLike[str]]"))]
    pub fn delete_many(&self, py: Python<'_>, paths: &Bound<'_, PyAny>) -> PyResult<()> {
        let paths = extract_delete_paths(paths)?;
        if paths.is_empty() {
            return Ok(());
        }
        let this = self.core.clone();
        py.detach(move || this.delete_iter(paths))
            .map_err(format_pyerr)
    }

    /// Check if a path exists.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to check.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if the path exists, False otherwise.
    pub fn exists(&self, py: Python<'_>, path: PathBuf) -> PyResult<bool> {
        let path = path.to_string_lossy().to_string();
        let this = self.core.clone();
        py.detach(move || this.exists(&path)).map_err(format_pyerr)
    }

    /// List entries in the given directory.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    /// limit : int, optional
    ///     The maximum number of entries to return.
    /// start_after : str, optional
    ///     The entry to start after.
    /// recursive : bool, optional
    ///     Whether to list recursively.
    /// versions : bool, optional
    ///     Whether to list versions.
    /// deleted : bool, optional
    ///     Whether to list deleted entries.
    ///
    /// Returns
    /// -------
    /// BlockingLister
    ///     An iterator over the entries in the directory.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        limit=None,
        start_after=None,
        recursive=None,
        versions=None,
        deleted=None) -> "collections.abc.Iterable[Entry]")]
    pub fn list(
        &self,
        py: Python<'_>,
        path: PathBuf,
        limit: Option<usize>,
        start_after: Option<String>,
        recursive: Option<bool>,
        versions: Option<bool>,
        deleted: Option<bool>,
    ) -> PyResult<BlockingLister> {
        let path = path.to_string_lossy().to_string();

        let opts = ListOptions {
            limit,
            start_after,
            recursive,
            versions,
            deleted,
        };

        let this = self.core.clone();
        let l = py
            .detach(move || this.lister_options(&path, opts.into()))
            .map_err(format_pyerr)?;
        Ok(BlockingLister::new(l))
    }

    /// Recursively list entries in the given directory.
    ///
    /// Deprecated
    /// ----------
    ///     Use `list()` with `recursive=True` instead.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    /// limit : int, optional
    ///     The maximum number of entries to return.
    /// start_after : str, optional
    ///     The entry to start after.
    /// versions : bool, optional
    ///     Whether to list versions.
    /// deleted : bool, optional
    ///     Whether to list deleted entries.
    ///
    /// Returns
    /// -------
    /// BlockingLister
    ///     An iterator over the entries in the directory.
    #[pyo3(signature = (path, *,
        limit=None,
        start_after=None,
        versions=None,
        deleted=None) -> "collections.abc.Iterable[Entry]")]
    pub fn scan(
        &self,
        py: Python<'_>,
        path: PathBuf,
        limit: Option<usize>,
        start_after: Option<String>,
        versions: Option<bool>,
        deleted: Option<bool>,
    ) -> PyResult<BlockingLister> {
        self.list(py, path, limit, start_after, Some(true), versions, deleted)
    }

    /// Get all capabilities of this operator.
    ///
    /// Returns
    /// -------
    /// Capability
    ///     The capability of the operator.
    pub fn capability(&self) -> PyResult<capability::Capability> {
        Ok(capability::Capability::new(self.core.info().capability()))
    }

    /// Check if the operator is able to work correctly.
    ///
    /// Raises
    /// ------
    /// Exception
    ///     If the operator is not able to work correctly.
    pub fn check(&self, py: Python<'_>) -> PyResult<()> {
        let this = self.core.clone();
        py.detach(move || this.check()).map_err(format_pyerr)
    }

    /// Create a new `AsyncOperator` from this blocking operator.
    ///
    /// Returns
    /// -------
    /// AsyncOperator
    ///     The async operator.
    pub fn to_async_operator(&self) -> PyResult<AsyncOperator> {
        Ok(AsyncOperator {
            core: self.core.clone().into(),
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    fn __repr__(&self) -> String {
        let info = self.core.info();
        let name = info.name();
        if name.is_empty() {
            format!("Operator(\"{}\", root=\"{}\")", info.scheme(), info.root())
        } else {
            format!(
                "Operator(\"{}\", root=\"{}\", name=\"{name}\")",
                info.scheme(),
                info.root()
            )
        }
    }
    fn __reduce__(&self, py: Python) -> PyResult<Py<PyAny>> {
        let reconstructor = py
            .import("opendal._opendal")?
            .getattr("_reconstruct_operator")?;
        let args = (self.__scheme.clone(), self.__map.clone()).into_py_any(py)?;
        PyTuple::new(py, [reconstructor.into_py_any(py)?, args])?.into_py_any(py)
    }
}

/// The async equivalent of `Operator`.
///
/// `AsyncOperator` is the entry point for all async APIs.
///
/// See also
/// --------
/// Operator
#[pyclass(module = "opendal.operator")]
pub struct AsyncOperator {
    core: ocore::Operator,
    __scheme: String,
    __map: HashMap<String, String>,
}
#[pymethods]
impl AsyncOperator {
    /// Create a new `AsyncOperator`.
    ///
    /// Parameters
    /// ----------
    /// scheme : str | Scheme
    ///     The scheme of the service.
    /// **kwargs : dict
    ///     The options for the service. Path-like values are converted to strings.
    ///
    /// Returns
    /// -------
    /// AsyncOperator
    ///     The new async operator.
    #[new]
    #[pyo3(signature = (scheme: "str | Scheme", *, **kwargs: "str | PathLike[str]"))]
    pub fn new(
        scheme: Bound<PyAny>,
        kwargs: Option<HashMap<String, Bound<PyAny>>>,
    ) -> PyResult<Self> {
        let scheme = if let Ok(scheme_str) = scheme.extract::<&str>() {
            scheme_str.to_string()
        } else if let Ok(py_scheme) = scheme.extract::<Scheme>() {
            String::from(py_scheme)
        } else {
            return Err(Unsupported::new_err(
                "Invalid type for scheme, expected str or Scheme",
            ));
        };
        let scheme = normalize_scheme(&scheme);

        let map = kwargs
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| {
                let value = value.extract::<String>().or_else(|_| {
                    value
                        .py()
                        .import("os")?
                        .getattr("fspath")?
                        .call1((&value,))?
                        .extract::<String>()
                })?;
                Ok((key, value))
            })
            .collect::<PyResult<HashMap<_, _>>>()?;

        Ok(AsyncOperator {
            core: build_operator(&scheme, map.clone())?,
            __scheme: scheme,
            __map: map,
        })
    }

    /// Create a new `AsyncOperator` from a URI string.
    ///
    /// The URI encodes the scheme and configuration in a single string, e.g.
    /// ``memory://`` or ``s3://bucket/path?region=us-east-1``. The scheme must
    /// belong to a service enabled in this build. Encode service options as
    /// query parameters; use ``urllib.parse.urlencode`` when building the URI
    /// dynamically.
    ///
    /// Parameters
    /// ----------
    /// uri : str
    ///     The URI of the service, including any options as query parameters.
    /// **kwargs : dict
    ///     Overrides for URI options. Prefer the URI query string.
    ///
    /// Returns
    /// -------
    /// AsyncOperator
    ///     The new async operator.
    ///
    /// Examples
    /// --------
    /// ```python
    /// from urllib.parse import urlencode
    /// import opendal
    ///
    /// op = opendal.AsyncOperator.from_uri("memory://")
    /// query = urlencode({"region": "us-east-1"})
    /// op = opendal.AsyncOperator.from_uri(f"s3://bucket/path?{query}")
    /// ```
    #[classmethod]
    #[pyo3(signature = (uri, **kwargs: "str"))]
    pub fn from_uri(
        _cls: &Bound<PyType>,
        uri: &str,
        kwargs: Option<HashMap<String, String>>,
    ) -> PyResult<Self> {
        let map = kwargs.unwrap_or_default();

        Ok(AsyncOperator {
            core: build_operator_from_uri(uri, map.clone())?,
            __scheme: uri.to_string(),
            __map: map,
        })
    }

    /// Create a new `AsyncOperator` from a typed service config.
    ///
    /// The config is an ``opendal.config.ServiceConfig`` (e.g. ``S3Config``); its
    /// ``scheme`` key selects the service, so a static type checker rejects a
    /// wrong scheme, a missing required key, an unknown key, and a wrong value
    /// type. Non-string values (``bool``, ``int``, ``os.PathLike``, ``list``)
    /// are converted to the string form core consumes.
    ///
    /// Parameters
    /// ----------
    /// config : ServiceConfig
    ///     A service configuration such as ``opendal.config.S3Config``.
    ///
    /// Returns
    /// -------
    /// AsyncOperator
    ///     The new async operator.
    ///
    /// Examples
    /// --------
    /// ```python
    /// import opendal
    /// from opendal.config import S3Config
    ///
    /// op = opendal.AsyncOperator.from_config(S3Config(scheme="s3", bucket="my-bucket"))
    /// ```
    #[classmethod]
    #[pyo3(signature = (config: "ServiceConfig"))]
    pub fn from_config(_cls: &Bound<PyType>, config: &Bound<PyAny>) -> PyResult<Self> {
        let (scheme, map) = extract_typed_config(config)?;
        let scheme = normalize_scheme(&scheme);

        Ok(AsyncOperator {
            core: build_operator(&scheme, map.clone())?,
            __scheme: scheme,
            __map: map,
        })
    }

    /// Add a new layer to the operator.
    ///
    /// Parameters
    /// ----------
    /// layer : Layer
    ///     The layer to add.
    ///
    /// Returns
    /// -------
    /// AsyncOperator
    ///     A new operator with the layer added.
    pub fn layer(&self, layer: &layers::Layer) -> PyResult<Self> {
        let op = layer.0.layer(self.core.clone());
        Ok(Self {
            core: op,
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    /// Open an async file-like object for the given path.
    ///
    /// The returning async file-like object is a context manager.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// mode : str
    ///     The mode to open the file in. Only "rb" and "wb" are supported.
    /// **kwargs : dict
    ///     Additional options for the underlying reader or writer.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns a file-like object.
    #[pyo3(signature = (path, mode, *, **kwargs: "Unpack[OpenKwargs]") -> "collections.abc.Awaitable[AsyncFile]")]
    pub fn open<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        mode: String,
        kwargs: Option<&Bound<PyDict>>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();

        let reader_opts = kwargs
            .map(|v| v.extract::<ReadOptions>())
            .transpose()?
            .unwrap_or_default();

        let writer_opts = kwargs
            .map(|v| v.extract::<WriteOptions>())
            .transpose()?
            .unwrap_or_default();

        future_into_py(py, async move {
            if mode == "rb" {
                let range = reader_opts.make_range();
                let reader = this
                    .reader_options(&path, reader_opts.into())
                    .await
                    .map_err(format_pyerr)?;

                let r = reader
                    .into_futures_async_read(range.to_range())
                    .await
                    .map_err(format_pyerr)?;
                Ok(AsyncFile::new_reader(r))
            } else if mode == "wb" {
                let mut writer_opts = writer_opts;
                // Keep small writes coalesced when neither the caller nor the
                // service supplies a chunk size, as the AsyncWrite adapter did.
                if writer_opts.chunk.is_none()
                    && this.info().capability().write_multi_min_size.is_none()
                {
                    writer_opts.chunk = Some(256 * 1024);
                }
                let writer = this
                    .writer_options(&path, writer_opts.into())
                    .await
                    .map_err(format_pyerr)?;
                Ok(AsyncFile::new_writer(writer))
            } else {
                Err(Unsupported::new_err(format!(
                    "OpenDAL doesn't support mode: {mode}"
                )))
            }
        })
    }

    /// Read the entire contents of a file at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// version : str, optional
    ///     The version of the file.
    /// concurrent : int, optional
    ///     The number of concurrent readers.
    /// chunk : int, optional
    ///     The size of each chunk.
    /// gap : int, optional
    ///     The gap between each chunk.
    /// offset : int, optional
    ///     The offset of the file.
    /// prefetch : int, optional
    ///     The number of bytes to prefetch.
    /// size : int, optional
    ///     The size of the file.
    /// if_match : str, optional
    ///     The ETag of the file.
    /// if_none_match : str, optional
    ///     The ETag of the file.
    /// if_modified_since : str, optional
    ///     The last modified time of the file.
    /// if_unmodified_since : str, optional
    ///     The last modified time of the file.
    /// content_type : str, optional
    ///     The content type of the file.
    /// cache_control : str, optional
    ///     The cache control of the file.
    /// content_disposition : str, optional
    ///     The content disposition of the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the contents of the file as bytes.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        version=None,
        concurrent=None,
        chunk=None,
        gap=None,
        offset=None,
        prefetch=None,
        size=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None) -> "collections.abc.Awaitable[bytes]")]
    pub fn read<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        version: Option<String>,
        concurrent: Option<usize>,
        chunk: Option<usize>,
        gap: Option<usize>,
        offset: Option<usize>,
        prefetch: Option<usize>,
        size: Option<usize>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = ReadOptions {
            version,
            concurrent,
            chunk,
            gap,
            offset,
            prefetch,
            size,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
        };
        future_into_py(py, async move {
            let range = opts.make_range();
            let res = this
                .reader_options(&path, opts.into())
                .await
                .map_err(format_pyerr)?
                .read(range.to_range())
                .await
                .map_err(format_pyerr)?;
            Python::attach(|py| buffer_into_py_bytes(py, res).map(Bound::unbind))
        })
    }

    /// Write bytes to a file at the given path.
    ///
    /// This function will create a file if it does not exist, and will
    /// overwrite its contents if it does.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// bs : bytes
    ///     The contents to write to the file.
    /// append : bool, optional
    ///     Whether to append to the file instead of overwriting it.
    /// chunk : int, optional
    ///     The chunk size to use when writing the file.
    /// concurrent : int, optional
    ///     The number of concurrent requests to make when writing the file.
    /// cache_control : str, optional
    ///     The cache control header to set on the file.
    /// content_type : str, optional
    ///     The content type header to set on the file.
    /// content_disposition : str, optional
    ///     The content disposition header to set on the file.
    /// content_encoding : str, optional
    ///     The content encoding header to set on the file.
    /// if_match : str, optional
    ///     The ETag to match when writing the file.
    /// if_none_match : str, optional
    ///     The ETag to not match when writing the file.
    /// if_not_exists : bool, optional
    ///     Whether to fail if the file already exists.
    /// user_metadata : dict, optional
    ///     The user metadata to set on the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the write is finished.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, bs: "bytes", *,
        append= None,
        chunk = None,
        concurrent = None,
        cache_control = None,
        content_type = None,
        content_disposition = None,
        content_encoding = None,
        if_match = None,
        if_none_match = None,
        if_not_exists = None,
        user_metadata = None) -> "collections.abc.Awaitable[None]")]
    pub fn write<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        bs: &Bound<PyAny>,
        append: Option<bool>,
        chunk: Option<usize>,
        concurrent: Option<usize>,
        cache_control: Option<String>,
        content_type: Option<String>,
        content_disposition: Option<String>,
        content_encoding: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_not_exists: Option<bool>,
        user_metadata: Option<HashMap<String, String>>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let opts = WriteOptions {
            append,
            chunk,
            concurrent,
            cache_control,
            content_type,
            content_disposition,
            content_encoding,
            if_match,
            if_none_match,
            if_not_exists,
            user_metadata,
        };
        let this = self.core.clone();
        let bs = py_bytes_like_into_buffer(bs)?;
        let path = path.to_string_lossy().to_string();
        future_into_py(py, async move {
            this.write_options(&path, bs, opts.into())
                .await
                .map(|_| ())
                .map_err(format_pyerr)
        })
    }

    /// Get the metadata of a file at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    /// version : str, optional
    ///     The version of the file.
    /// if_match : str, optional
    ///     The ETag of the file.
    /// if_none_match : str, optional
    ///     The ETag of the file.
    /// if_modified_since : datetime, optional
    ///     The last modified time of the file.
    /// if_unmodified_since : datetime, optional
    ///     The last modified time of the file.
    /// content_type : str, optional
    ///     The content type of the file.
    /// cache_control : str, optional
    ///     The cache control of the file.
    /// content_disposition : str, optional
    ///     The content disposition of the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the metadata of the file.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        version=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None) -> "collections.abc.Awaitable[Metadata]")]
    pub fn stat<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        version: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = StatOptions {
            version,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
        };

        future_into_py(py, async move {
            let res: Metadata = this
                .stat_options(&path, opts.into())
                .await
                .map_err(format_pyerr)
                .map(Metadata::new)?;

            Ok(res)
        })
    }

    /// Copy a file from one path to another.
    ///
    /// Parameters
    /// ----------
    /// source : str
    ///     The path to the source file.
    /// target : str
    ///     The path to the target file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the copy is finished.
    #[pyo3(signature = (source, target) -> "collections.abc.Awaitable[None]")]
    pub fn copy<'p>(
        &'p self,
        py: Python<'p>,
        source: PathBuf,
        target: PathBuf,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let source = source.to_string_lossy().to_string();
        let target = target.to_string_lossy().to_string();
        future_into_py(py, async move {
            this.copy(&source, &target)
                .await
                .map(|_| ())
                .map_err(format_pyerr)
        })
    }

    /// Rename (move) a file from one path to another.
    ///
    /// Parameters
    /// ----------
    /// source : str
    ///     The path to the source file.
    /// target : str
    ///     The path to the target file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the rename is finished.
    #[pyo3(signature = (source, target) -> "collections.abc.Awaitable[None]")]
    pub fn rename<'p>(
        &'p self,
        py: Python<'p>,
        source: PathBuf,
        target: PathBuf,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let source = source.to_string_lossy().to_string();
        let target = target.to_string_lossy().to_string();
        future_into_py(py, async move {
            this.rename(&source, &target).await.map_err(format_pyerr)
        })
    }

    /// Recursively remove all files and directories at the given path.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to remove.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the removal is finished.
    #[pyo3(signature = (path) -> "collections.abc.Awaitable[None]")]
    pub fn remove_all<'p>(&'p self, py: Python<'p>, path: PathBuf) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        future_into_py(py, async move {
            this.delete_with(&path)
                .recursive(true)
                .await
                .map_err(format_pyerr)
        })
    }

    /// Check if the operator is able to work correctly.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the check is finished.
    ///
    /// Raises
    /// ------
    /// Exception
    ///     If the operator is not able to work correctly.
    #[pyo3(signature = () -> "collections.abc.Awaitable[None]")]
    pub fn check<'p>(&'p self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move { this.check().await.map_err(format_pyerr) })
    }

    /// Create a directory at the given path.
    ///
    /// Notes
    /// -----
    /// To indicate that a path is a directory, it must end with a `/`.
    /// This operation is always recursive, like `mkdir -p`.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the directory is created.
    #[pyo3(signature = (path) -> "collections.abc.Awaitable[None]")]
    pub fn create_dir<'p>(&'p self, py: Python<'p>, path: PathBuf) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        future_into_py(py, async move {
            this.create_dir(&path).await.map_err(format_pyerr)
        })
    }

    /// Delete a file at the given path.
    ///
    /// Notes
    /// -----
    /// This operation will not return an error if the path does not exist.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that completes when the file is deleted.
    /// version : str, optional
    ///     The version of the file to delete. Only supported on version-aware backends.
    /// recursive : bool, optional
    ///     If True, delete the path recursively.
    ///     Only supported on backends that support recursive delete.
    /// if_match : str, optional
    ///     If set, only delete when the existing object's ETag matches.
    #[pyo3(signature = (path, *, version=None, recursive=None, if_match=None) -> "collections.abc.Awaitable[None]")]
    pub fn delete<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        version: Option<String>,
        recursive: Option<bool>,
        if_match: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        future_into_py(py, async move {
            if version.is_some() || recursive.is_some() || if_match.is_some() {
                let opts = ocore::options::DeleteOptions {
                    version,
                    recursive: recursive.unwrap_or(false),
                    if_match,
                    ..Default::default()
                };
                this.delete_options(&path, opts).await.map_err(format_pyerr)
            } else {
                this.delete(&path).await.map_err(format_pyerr)
            }
        })
    }

    /// Delete an explicit collection of paths with default delete options.
    ///
    /// Notes
    /// -----
    /// This method collects the iterable and converts its Python path objects
    /// before deletion starts, using memory proportional to the number of paths.
    /// Empty collections do nothing. Paths that do not exist are ignored.
    /// Storage or path-validation errors can leave some paths deleted; this
    /// operation is not atomic and does not guarantee a single remote request.
    /// Use `remove_all` to recursively delete a path prefix.
    ///
    /// Parameters
    /// ----------
    /// paths : Iterable[str | PathLike[str]]
    ///     The paths to delete. A single path is not accepted.
    ///
    /// Returns
    /// -------
    /// Awaitable[None]
    ///     An awaitable that completes when deletion finishes.
    #[pyo3(signature = (paths: "collections.abc.Iterable[str | PathLike[str]]") -> "collections.abc.Awaitable[None]")]
    pub fn delete_many<'p>(
        &'p self,
        py: Python<'p>,
        paths: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let paths = extract_delete_paths(paths)?;
        let this = self.core.clone();
        future_into_py(py, async move {
            if paths.is_empty() {
                return Ok(());
            }
            this.delete_iter(paths).await.map_err(format_pyerr)
        })
    }

    /// Check if a path exists.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to check.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns True if the path exists, False otherwise.
    #[pyo3(signature = (path) -> "collections.abc.Awaitable[bool]")]
    pub fn exists<'p>(&'p self, py: Python<'p>, path: PathBuf) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        future_into_py(
            py,
            async move { this.exists(&path).await.map_err(format_pyerr) },
        )
    }

    /// List entries in the given directory.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    /// limit : int, optional
    ///     The maximum number of entries to return.
    /// start_after : str, optional
    ///     The entry to start after.
    /// recursive : bool, optional
    ///     Whether to list recursively.
    /// versions : bool, optional
    ///     Whether to list versions.
    /// deleted : bool, optional
    ///     Whether to list deleted entries.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns an async iterator over the entries.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, *,
        limit=None,
        start_after=None,
        recursive=None,
        versions=None,
        deleted=None) -> "collections.abc.Awaitable[collections.abc.AsyncIterable[Entry]]")]
    pub fn list<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        limit: Option<usize>,
        start_after: Option<String>,
        recursive: Option<bool>,
        versions: Option<bool>,
        deleted: Option<bool>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = ListOptions {
            limit,
            start_after,
            recursive,
            versions,
            deleted,
        };

        future_into_py(py, async move {
            let lister = this
                .lister_options(&path, opts.into())
                .await
                .map_err(format_pyerr)?;
            let pylister = Python::attach(|py| AsyncLister::new(lister).into_py_any(py))?;

            Ok(pylister)
        })
    }

    /// Recursively list entries in the given directory.
    ///
    /// Deprecated
    /// ----------
    ///     Use `list()` with `recursive=True` instead.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path to the directory.
    /// limit : int, optional
    ///     The maximum number of entries to return.
    /// start_after : str, optional
    ///     The entry to start after.
    /// versions : bool, optional
    ///     Whether to list versions.
    /// deleted : bool, optional
    ///     Whether to list deleted entries.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns an async iterator over the entries.
    #[pyo3(signature = (path, *,
        limit=None,
        start_after=None,
        versions=None,
        deleted=None) -> "collections.abc.Awaitable[collections.abc.AsyncIterable[Entry]]")]
    pub fn scan<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        limit: Option<usize>,
        start_after: Option<String>,
        versions: Option<bool>,
        deleted: Option<bool>,
    ) -> PyResult<Bound<'p, PyAny>> {
        self.list(py, path, limit, start_after, Some(true), versions, deleted)
    }

    /// Create a presigned request for a stat operation.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path of the object to stat.
    /// expire_second : int
    ///     The number of seconds until the presigned URL expires.
    /// version : str, optional
    ///     The version of the file.
    /// if_match : str, optional
    ///     The ETag to match.
    /// if_none_match : str, optional
    ///     The ETag to not match.
    /// if_modified_since : datetime, optional
    ///     Only return if modified since this time.
    /// if_unmodified_since : datetime, optional
    ///     Only return if unmodified since this time.
    /// content_type : str, optional
    ///     Override the content type in the presigned response.
    /// cache_control : str, optional
    ///     Override the cache control in the presigned response.
    /// content_disposition : str, optional
    ///     Override the content disposition in the presigned response.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns a presigned request object.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, expire_second, *,
        version=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None) -> "collections.abc.Awaitable[PresignedRequest]")]
    pub fn presign_stat<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        expire_second: u64,
        version: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = StatOptions {
            version,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
        };
        future_into_py(py, async move {
            let res = this
                .presign_stat_options(&path, Duration::from_secs(expire_second), opts.into())
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Create a presigned request for a read operation.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path of the object to read.
    /// expire_second : int
    ///     The number of seconds until the presigned URL expires.
    /// version : str, optional
    ///     The version of the file.
    /// if_match : str, optional
    ///     The ETag to match.
    /// if_none_match : str, optional
    ///     The ETag to not match.
    /// if_modified_since : datetime, optional
    ///     Only return if modified since this time.
    /// if_unmodified_since : datetime, optional
    ///     Only return if unmodified since this time.
    /// content_type : str, optional
    ///     Override the content type in the presigned response.
    /// cache_control : str, optional
    ///     Override the cache control in the presigned response.
    /// content_disposition : str, optional
    ///     Override the content disposition in the presigned response.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns a presigned request object.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, expire_second, *,
        version=None,
        if_match=None,
        if_none_match=None,
        if_modified_since=None,
        if_unmodified_since=None,
        content_type=None,
        cache_control=None,
        content_disposition=None) -> "collections.abc.Awaitable[PresignedRequest]")]
    pub fn presign_read<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        expire_second: u64,
        version: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_modified_since: Option<jiff::Timestamp>,
        if_unmodified_since: Option<jiff::Timestamp>,
        content_type: Option<String>,
        cache_control: Option<String>,
        content_disposition: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = ReadOptions {
            version,
            if_match,
            if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_type,
            cache_control,
            content_disposition,
            ..Default::default()
        };
        future_into_py(py, async move {
            let res = this
                .presign_read_options(&path, Duration::from_secs(expire_second), opts.into())
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Create a presigned request for a write operation.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path of the object to write to.
    /// expire_second : int
    ///     The number of seconds until the presigned URL expires.
    /// content_type : str, optional
    ///     The content type header to set on the file.
    /// content_disposition : str, optional
    ///     The content disposition header to set on the file.
    /// content_encoding : str, optional
    ///     The content encoding header to set on the file.
    /// cache_control : str, optional
    ///     The cache control header to set on the file.
    /// if_match : str, optional
    ///     The ETag to match when writing the file.
    /// if_none_match : str, optional
    ///     The ETag to not match when writing the file.
    /// if_not_exists : bool, optional
    ///     Whether to fail if the file already exists.
    /// user_metadata : dict, optional
    ///     The user metadata to set on the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns a presigned request object.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (path, expire_second, *,
        content_type=None,
        content_disposition=None,
        content_encoding=None,
        cache_control=None,
        if_match=None,
        if_none_match=None,
        if_not_exists=None,
        user_metadata=None) -> "collections.abc.Awaitable[PresignedRequest]")]
    pub fn presign_write<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        expire_second: u64,
        content_type: Option<String>,
        content_disposition: Option<String>,
        content_encoding: Option<String>,
        cache_control: Option<String>,
        if_match: Option<String>,
        if_none_match: Option<String>,
        if_not_exists: Option<bool>,
        user_metadata: Option<HashMap<String, String>>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = WriteOptions {
            content_type,
            content_disposition,
            content_encoding,
            cache_control,
            if_match,
            if_none_match,
            if_not_exists,
            user_metadata,
            ..Default::default()
        };
        future_into_py(py, async move {
            let res = this
                .presign_write_options(&path, Duration::from_secs(expire_second), opts.into())
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Create a presigned request for a delete operation.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///     The path of the object to delete.
    /// expire_second : int
    ///     The number of seconds until the presigned URL expires.
    /// version : str, optional
    ///     The version of the file to delete.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns a presigned request object.
    #[pyo3(signature = (path, expire_second, *, version=None) -> "collections.abc.Awaitable[PresignedRequest]")]
    pub fn presign_delete<'p>(
        &'p self,
        py: Python<'p>,
        path: PathBuf,
        expire_second: u64,
        version: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        let path = path.to_string_lossy().to_string();
        let opts = DeleteOptions {
            version,
            ..Default::default()
        };
        future_into_py(py, async move {
            let res = this
                .presign_delete_options(&path, Duration::from_secs(expire_second), opts.into())
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Get all capabilities of this operator.
    ///
    /// Returns
    /// -------
    /// Capability
    ///     The capability of the operator.
    pub fn capability(&self) -> PyResult<Capability> {
        Ok(capability::Capability::new(self.core.info().capability()))
    }

    /// Create a new blocking `Operator` from this async operator.
    ///
    /// Returns
    /// -------
    /// Operator
    ///     The blocking operator.
    pub fn to_operator(&self) -> PyResult<Operator> {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        let _guard = runtime.enter();
        let op = ocore::blocking::Operator::new(self.core.clone()).map_err(format_pyerr)?;

        Ok(Operator {
            core: op,
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    fn __repr__(&self) -> String {
        let info = self.core.info();
        let name = info.name();
        if name.is_empty() {
            format!(
                "AsyncOperator(\"{}\", root=\"{}\")",
                info.scheme(),
                info.root()
            )
        } else {
            format!(
                "AsyncOperator(\"{}\", root=\"{}\", name=\"{name}\")",
                info.scheme(),
                info.root()
            )
        }
    }
    fn __reduce__(&self, py: Python) -> PyResult<Py<PyAny>> {
        let reconstructor = py
            .import("opendal._opendal")?
            .getattr("_reconstruct_async_operator")?;
        let args = (self.__scheme.clone(), self.__map.clone()).into_py_any(py)?;
        PyTuple::new(py, [reconstructor.into_py_any(py)?, args])?.into_py_any(py)
    }
}

/// A presigned request.
///
/// This contains the information required to make a request to the
/// underlying service, including the URL, method, and headers.
#[pyclass(module = "opendal.types")]
pub struct PresignedRequest(ocore::raw::PresignedRequest);
#[pymethods]
impl PresignedRequest {
    /// The URL of this request.
    #[getter]
    pub fn url(&self) -> String {
        self.0.uri().to_string()
    }

    /// The HTTP method of this request.
    #[getter]
    pub fn method(&self) -> &str {
        self.0.method().as_str()
    }

    /// The HTTP headers of this request.
    ///
    /// Returns
    /// -------
    /// dict
    ///     The HTTP headers of this request.
    #[getter]
    pub fn headers(&self) -> PyResult<HashMap<&str, &str>> {
        let mut headers = HashMap::new();
        for (k, v) in self.0.header().iter() {
            let k = k.as_str();
            let v = v
                .to_str()
                .map_err(|err| Unexpected::new_err(err.to_string()))?;
            if headers.insert(k, v).is_some() {
                return Err(Unexpected::new_err("duplicate header"));
            }
        }
        Ok(headers)
    }
}
