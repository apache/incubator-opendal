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
use std::str::FromStr;
use std::time::Duration;

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;
use pyo3::IntoPyObjectExt;
use pyo3_async_runtimes::tokio::future_into_py;

use crate::*;

fn build_operator(
    scheme: ocore::Scheme,
    map: HashMap<String, String>,
) -> PyResult<ocore::Operator> {
    let mut op = ocore::Operator::via_iter(scheme, map).map_err(format_pyerr)?;
    if !op.info().full_capability().blocking {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        let _guard = runtime.enter();
        op = op
            .layer(ocore::layers::BlockingLayer::create().expect("blocking layer must be created"));
    }

    Ok(op)
}

/// `Operator` is the entry for all public blocking APIs
///
/// Create a new blocking `Operator` with the given `scheme` and options(`**kwargs`).
#[pyclass(module = "opendal")]
pub struct Operator {
    core: ocore::BlockingOperator,
    __scheme: ocore::Scheme,
    __map: HashMap<String, String>,
}

#[pymethods]
impl Operator {
    #[new]
    #[pyo3(signature = (scheme, *, **map))]
    pub fn new(scheme: &str, map: Option<&Bound<PyDict>>) -> PyResult<Self> {
        let scheme = ocore::Scheme::from_str(scheme)
            .map_err(|err| {
                ocore::Error::new(ocore::ErrorKind::Unexpected, "unsupported scheme")
                    .set_source(err)
            })
            .map_err(format_pyerr)?;
        let map = map
            .map(|v| {
                v.extract::<HashMap<String, String>>()
                    .expect("must be valid hashmap")
            })
            .unwrap_or_default();

        Ok(Operator {
            core: build_operator(scheme.clone(), map.clone())?.blocking(),
            __scheme: scheme,
            __map: map,
        })
    }

    /// Add new layers upon existing operator
    pub fn layer(&self, layer: &layers::Layer) -> PyResult<Self> {
        let op = layer.0.layer(self.core.clone().into());
        Ok(Self {
            core: op.blocking(),
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    /// Open a file-like reader for the given path.
    pub fn open(&self, path: String, mode: String) -> PyResult<File> {
        let this = self.core.clone();
        if mode == "rb" {
            let r = this
                .reader(&path)
                .map_err(format_pyerr)?
                .into_std_read(..)
                .map_err(format_pyerr)?;
            Ok(File::new_reader(r))
        } else if mode == "wb" {
            let w = this.writer(&path).map_err(format_pyerr)?;
            Ok(File::new_writer(w))
        } else {
            Err(Unsupported::new_err(format!(
                "OpenDAL doesn't support mode: {mode}"
            )))
        }
    }

    /// Read the whole path into bytes.
    pub fn read<'p>(&'p self, py: Python<'p>, path: &str) -> PyResult<Bound<'p, PyAny>> {
        let buffer = self.core.read(path).map_err(format_pyerr)?.to_vec();
        Buffer::new(buffer).into_bytes_ref(py)
    }

    /// Write bytes into given path.
    #[pyo3(signature = (path, bs, **kwargs))]
    pub fn write(&self, path: &str, bs: Vec<u8>, kwargs: Option<WriteOptions>) -> PyResult<()> {
        let kwargs = kwargs.unwrap_or_default();
        let mut write = self
            .core
            .write_with(path, bs)
            .append(kwargs.append.unwrap_or(false));
        if let Some(chunk) = kwargs.chunk {
            write = write.chunk(chunk);
        }
        if let Some(content_type) = &kwargs.content_type {
            write = write.content_type(content_type);
        }
        if let Some(content_disposition) = &kwargs.content_disposition {
            write = write.content_disposition(content_disposition);
        }
        if let Some(cache_control) = &kwargs.cache_control {
            write = write.cache_control(cache_control);
        }

        write.call().map(|_| ()).map_err(format_pyerr)
    }

    /// Get current path's metadata **without cache** directly.
    pub fn stat(&self, path: &str) -> PyResult<Metadata> {
        self.core
            .stat(path)
            .map_err(format_pyerr)
            .map(Metadata::new)
    }

    /// Copy source to target.
    pub fn copy(&self, source: &str, target: &str) -> PyResult<()> {
        self.core.copy(source, target).map_err(format_pyerr)
    }

    /// Rename filename.
    pub fn rename(&self, source: &str, target: &str) -> PyResult<()> {
        self.core.rename(source, target).map_err(format_pyerr)
    }

    /// Remove all file
    pub fn remove_all(&self, path: &str) -> PyResult<()> {
        self.core.remove_all(path).map_err(format_pyerr)
    }

    /// Create a dir at given path.
    ///
    /// # Notes
    ///
    /// To indicate that a path is a directory, it is compulsory to include
    /// a trailing / in the path. Failure to do so may result in
    /// `NotADirectory` error being returned by OpenDAL.
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create dir is always recursive, works like `mkdir -p`
    pub fn create_dir(&self, path: &str) -> PyResult<()> {
        self.core.create_dir(path).map_err(format_pyerr)
    }

    /// Delete given path.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    pub fn delete(&self, path: &str) -> PyResult<()> {
        self.core.delete(path).map_err(format_pyerr)
    }

    /// List current dir path.
    pub fn list(&self, path: &str) -> PyResult<BlockingLister> {
        let l = self.core.lister(path).map_err(format_pyerr)?;
        Ok(BlockingLister::new(l))
    }

    /// List dir in flat way.
    pub fn scan(&self, path: &str) -> PyResult<BlockingLister> {
        let l = self
            .core
            .lister_with(path)
            .recursive(true)
            .call()
            .map_err(format_pyerr)?;
        Ok(BlockingLister::new(l))
    }

    pub fn capability(&self) -> PyResult<capability::Capability> {
        Ok(capability::Capability::new(
            self.core.info().full_capability(),
        ))
    }

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

    fn __getnewargs_ex__(&self, py: Python) -> PyResult<PyObject> {
        let args = vec![self.__scheme.to_string()];
        let args = PyTuple::new(py, args)?.into_py_any(py)?;
        let kwargs = self.__map.clone().into_py_any(py)?;
        Ok(PyTuple::new(py, [args, kwargs])?.into_py_any(py)?)
    }
}

/// `AsyncOperator` is the entry for all public async APIs
///
/// Create a new `AsyncOperator` with the given `scheme` and options(`**kwargs`).
#[pyclass(module = "opendal")]
pub struct AsyncOperator {
    core: ocore::Operator,
    __scheme: ocore::Scheme,
    __map: HashMap<String, String>,
}

#[pymethods]
impl AsyncOperator {
    #[new]
    #[pyo3(signature = (scheme, *,  **map))]
    pub fn new(scheme: &str, map: Option<&Bound<PyDict>>) -> PyResult<Self> {
        let scheme = ocore::Scheme::from_str(scheme)
            .map_err(|err| {
                ocore::Error::new(ocore::ErrorKind::Unexpected, "unsupported scheme")
                    .set_source(err)
            })
            .map_err(format_pyerr)?;
        let map = map
            .map(|v| {
                v.extract::<HashMap<String, String>>()
                    .expect("must be valid hashmap")
            })
            .unwrap_or_default();

        Ok(AsyncOperator {
            core: build_operator(scheme.clone(), map.clone())?.into(),
            __scheme: scheme,
            __map: map,
        })
    }

    /// Add new layers upon existing operator
    pub fn layer(&self, layer: &layers::Layer) -> PyResult<Self> {
        let op = layer.0.layer(self.core.clone());
        Ok(Self {
            core: op,
            __scheme: self.__scheme.clone(),
            __map: self.__map.clone(),
        })
    }

    /// Open a file-like reader for the given path.
    pub fn open<'p>(
        &'p self,
        py: Python<'p>,
        path: String,
        mode: String,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();

        future_into_py(py, async move {
            if mode == "rb" {
                let r = this
                    .reader(&path)
                    .await
                    .map_err(format_pyerr)?
                    .into_futures_async_read(..)
                    .await
                    .map_err(format_pyerr)?;
                Ok(AsyncFile::new_reader(r))
            } else if mode == "wb" {
                let w = this.writer(&path).await.map_err(format_pyerr)?;
                Ok(AsyncFile::new_writer(w))
            } else {
                Err(Unsupported::new_err(format!(
                    "OpenDAL doesn't support mode: {mode}"
                )))
            }
        })
    }

    /// Read the whole path into bytes.
    pub fn read<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let res: Vec<u8> = this.read(&path).await.map_err(format_pyerr)?.to_vec();
            Python::with_gil(|py| Buffer::new(res).into_bytes(py))
        })
    }

    /// Write bytes into given path.
    #[pyo3(signature = (path, bs, **kwargs))]
    pub fn write<'p>(
        &'p self,
        py: Python<'p>,
        path: String,
        bs: &Bound<PyBytes>,
        kwargs: Option<WriteOptions>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let kwargs = kwargs.unwrap_or_default();
        let this = self.core.clone();
        let bs = bs.as_bytes().to_vec();
        future_into_py(py, async move {
            let mut write = this
                .write_with(&path, bs)
                .append(kwargs.append.unwrap_or(false));
            if let Some(buffer) = kwargs.chunk {
                write = write.chunk(buffer);
            }
            if let Some(content_type) = &kwargs.content_type {
                write = write.content_type(content_type);
            }
            if let Some(content_disposition) = &kwargs.content_disposition {
                write = write.content_disposition(content_disposition);
            }
            if let Some(cache_control) = &kwargs.cache_control {
                write = write.cache_control(cache_control);
            }
            write.await.map(|_| ()).map_err(format_pyerr)
        })
    }

    /// Get current path's metadata **without cache** directly.
    pub fn stat<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let res: Metadata = this
                .stat(&path)
                .await
                .map_err(format_pyerr)
                .map(Metadata::new)?;

            Ok(res)
        })
    }

    /// Copy source to target.``
    pub fn copy<'p>(
        &'p self,
        py: Python<'p>,
        source: String,
        target: String,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            this.copy(&source, &target).await.map_err(format_pyerr)
        })
    }

    /// Rename filename
    pub fn rename<'p>(
        &'p self,
        py: Python<'p>,
        source: String,
        target: String,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            this.rename(&source, &target).await.map_err(format_pyerr)
        })
    }

    /// Remove all file
    pub fn remove_all<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            this.remove_all(&path).await.map_err(format_pyerr)
        })
    }

    /// Create a dir at given path.
    ///
    /// # Notes
    ///
    /// To indicate that a path is a directory, it is compulsory to include
    /// a trailing / in the path. Failure to do so may result in
    /// `NotADirectory` error being returned by OpenDAL.
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create dir is always recursive, works like `mkdir -p`
    pub fn create_dir<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            this.create_dir(&path).await.map_err(format_pyerr)
        })
    }

    /// Delete given path.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    pub fn delete<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(
            py,
            async move { this.delete(&path).await.map_err(format_pyerr) },
        )
    }

    /// List current dir path.
    pub fn list<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let lister = this.lister(&path).await.map_err(format_pyerr)?;
            let pylister = Python::with_gil(|py| AsyncLister::new(lister).into_py_any(py))?;

            Ok(pylister)
        })
    }

    /// List dir in flat way.
    pub fn scan<'p>(&'p self, py: Python<'p>, path: String) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let lister = this
                .lister_with(&path)
                .recursive(true)
                .await
                .map_err(format_pyerr)?;
            let pylister: PyObject =
                Python::with_gil(|py| AsyncLister::new(lister).into_py_any(py))?;
            Ok(pylister)
        })
    }

    /// Presign an operation for stat(head) which expires after `expire_second` seconds.
    pub fn presign_stat<'p>(
        &'p self,
        py: Python<'p>,
        path: String,
        expire_second: u64,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let res = this
                .presign_stat(&path, Duration::from_secs(expire_second))
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Presign an operation for read which expires after `expire_second` seconds.
    pub fn presign_read<'p>(
        &'p self,
        py: Python<'p>,
        path: String,
        expire_second: u64,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let res = this
                .presign_read(&path, Duration::from_secs(expire_second))
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    /// Presign an operation for write which expires after `expire_second` seconds.
    pub fn presign_write<'p>(
        &'p self,
        py: Python<'p>,
        path: String,
        expire_second: u64,
    ) -> PyResult<Bound<'p, PyAny>> {
        let this = self.core.clone();
        future_into_py(py, async move {
            let res = this
                .presign_write(&path, Duration::from_secs(expire_second))
                .await
                .map_err(format_pyerr)
                .map(PresignedRequest)?;

            Ok(res)
        })
    }

    pub fn capability(&self) -> PyResult<capability::Capability> {
        Ok(capability::Capability::new(
            self.core.info().full_capability(),
        ))
    }

    pub fn to_operator(&self) -> PyResult<Operator> {
        Ok(Operator {
            core: self.core.clone().blocking(),
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

    fn __getnewargs_ex__(&self, py: Python) -> PyResult<PyObject> {
        let args = vec![self.__scheme.to_string()];
        let args = PyTuple::new(py, args)?.into_py_any(py)?;
        let kwargs = self.__map.clone().into_py_any(py)?;
        Ok(PyTuple::new(py, [args, kwargs])?.into_py_any(py)?)
    }
}

#[pyclass(module = "opendal")]
pub struct PresignedRequest(ocore::raw::PresignedRequest);

#[pymethods]
impl PresignedRequest {
    /// Return the URL of this request.
    #[getter]
    pub fn url(&self) -> String {
        self.0.uri().to_string()
    }

    /// Return the HTTP method of this request.
    #[getter]
    pub fn method(&self) -> &str {
        self.0.method().as_str()
    }

    /// Return the HTTP headers of this request.
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
