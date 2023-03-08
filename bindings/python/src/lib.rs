// Copyright 2022 Datafuse Labs
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

use std::collections::HashMap;
use std::str::FromStr;

use ::opendal as od;
use pyo3::exceptions::{PyFileNotFoundError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3_asyncio::tokio::future_into_py;

fn build_operator(scheme: od::Scheme, map: HashMap<String, String>) -> PyResult<od::Operator> {
    use od::services::*;

    let op = match scheme {
        od::Scheme::Memory => od::Operator::from_map::<Memory>(map)
            .map_err(format_pyerr)?
            .finish(),
        od::Scheme::Fs => od::Operator::from_map::<Fs>(map)
            .map_err(format_pyerr)?
            .finish(),
        _ => unimplemented!("not supported"),
    };

    Ok(op)
}

#[pyclass]
struct AsyncOperator(od::Operator);

#[pymethods]
impl AsyncOperator {
    #[new]
    #[pyo3(signature = (scheme, **map))]
    pub fn new(scheme: &str, map: Option<&PyDict>) -> PyResult<Self> {
        let scheme = od::Scheme::from_str(scheme)
            .map_err(|err| {
                od::Error::new(od::ErrorKind::Unexpected, "not supported scheme").set_source(err)
            })
            .map_err(format_pyerr)?;
        let map = map
            .map(|v| {
                v.extract::<HashMap<String, String>>()
                    .expect("must be valid hashmap")
            })
            .unwrap_or_default();

        Ok(AsyncOperator(build_operator(scheme, map)?))
    }

    pub fn read<'p>(&'p self, py: Python<'p>, path: &'p str) -> PyResult<&'p PyAny> {
        let this = self.0.clone();
        let path = path.to_string();
        future_into_py(py, async move {
            let res = this.read(&path).await.map_err(format_pyerr)?;
            let bytes = Python::with_gil(|py| PyBytes::new(py, &res).to_object(py));
            Ok(bytes)
        })
    }

    pub fn write<'p>(&'p self, py: Python<'p>, path: &'p str, bs: Vec<u8>) -> PyResult<&'p PyAny> {
        let this = self.0.clone();
        let path = path.to_string();
        future_into_py(py, async move {
            this.write(&path, bs).await.map_err(format_pyerr)
        })
    }

    pub fn stat<'p>(&'p self, py: Python<'p>, path: &'p str) -> PyResult<&'p PyAny> {
        let this = self.0.clone();
        let path = path.to_string();
        future_into_py(py, async move {
            let res: Metadata = this.stat(&path).await.map_err(format_pyerr).map(Metadata)?;

            Ok(res)
        })
    }
}

#[pyclass]
struct Operator(od::BlockingOperator);

#[pymethods]
impl Operator {
    #[new]
    #[pyo3(signature = (scheme="", **map))]
    pub fn new(scheme: &str, map: Option<&PyDict>) -> PyResult<Self> {
        let scheme = od::Scheme::from_str(scheme)
            .map_err(|err| {
                od::Error::new(od::ErrorKind::Unexpected, "not supported scheme").set_source(err)
            })
            .map_err(format_pyerr)?;
        let map = map
            .map(|v| {
                v.extract::<HashMap<String, String>>()
                    .expect("must be valid hashmap")
            })
            .unwrap_or_default();

        Ok(Operator(build_operator(scheme, map)?.blocking()))
    }

    pub fn read<'p>(&'p self, py: Python<'p>, path: &str) -> PyResult<&'p PyBytes> {
        let res = self.0.read(path).map_err(format_pyerr)?;
        let bytes = PyBytes::new(py, &res);
        Ok(bytes)
    }

    pub fn write(&self, path: &str, bs: Vec<u8>) -> PyResult<()> {
        self.0.write(path, bs).map_err(format_pyerr)
    }

    pub fn stat(&self, path: &str) -> PyResult<Metadata> {
        self.0.stat(path).map_err(format_pyerr).map(Metadata)
    }
}

#[pyclass]
struct Metadata(od::Metadata);

#[pymethods]
impl Metadata {
    pub fn content_length(&self) -> u64 {
        self.0.content_length()
    }
}

fn format_pyerr(err: od::Error) -> PyErr {
    use od::ErrorKind::*;
    match err.kind() {
        NotFound => PyFileNotFoundError::new_err(err.to_string()),
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

#[pymodule]
fn opendal(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Operator>()?;
    m.add_class::<AsyncOperator>()?;
    Ok(())
}
