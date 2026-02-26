//!
//! RustStack Python Bindings
//!
//! # Installation
//!
//! ```bash
//! cd ruststack-py
//! pip install maturin
//! maturin develop
//! ```
//!
//! # Usage
//!
//! ```python
//! import ruststack_py
//!
//! rs = ruststack_py.RustStack()
//! ```
//!
//! Note: Full in-process bindings are a work in progress.
//! For now, use the Docker-based tests for full coverage.

#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;

#[pyclass]
pub struct RustStack {
    // Placeholder - full implementation requires async trait support
    _marker: std::marker::PhantomData<()>,
}

#[pymethods]
impl RustStack {
    #[new]
    fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Note: Full in-process bindings are under development.
    ///
    /// For complete functionality, use Docker-based testing:
    /// ```bash
    /// docker run -p 4566:4566 ghcr.io/eddalmond/ruststack:latest
    /// ```
    fn ddb_create_table(&self, _name: &str, _key_attr: &str, _key_type: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker: docker run -p 4566:4566 ghcr.io/eddalmond/ruststack:latest"
        ))
    }

    fn ddb_put_item(&self, _table_name: &str, _key: &str, _value: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn ddb_get_item(&self, _table_name: &str, _key: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn ddb_list_tables(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn ddb_delete_table(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_create_bucket(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_put_object(&self, _bucket: &str, _key: &str, _value: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_get_object(&self, _bucket: &str, _key: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_list_objects(&self, _bucket: &str) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_delete_object(&self, _bucket: &str, _key: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn s3_delete_bucket(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn secrets_create_secret(&self, _name: &str, _value: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn secrets_get_secret_value(&self, _name: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn secrets_put_secret_value(&self, _name: &str, _value: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn secrets_delete_secret(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn secrets_list_secrets(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn firehose_create_delivery_stream(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn firehose_put_record(&self, _stream_name: &str, _data: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn firehose_put_record_batch(
        &self,
        _stream_name: &str,
        _records: Vec<String>,
    ) -> PyResult<usize> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn firehose_delete_delivery_stream(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn iam_create_role(&self, _name: &str, _policy: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn iam_get_role(&self, _name: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn iam_list_roles(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn iam_delete_role(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sns_create_topic(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sns_publish(&self, _topic: &str, _message: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sns_subscribe(&self, _topic: &str, _protocol: &str, _endpoint: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sns_list_topics(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sns_delete_topic(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sqs_create_queue(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sqs_send_message(&self, _queue: &str, _body: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sqs_receive_message(&self, _queue: &str, _max_messages: i32) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sqs_delete_message(&self, _queue: &str, _receipt_handle: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    #[pyo3(signature = (_prefix = None,))]
    fn sqs_list_queues(&self, _prefix: Option<String>) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }

    fn sqs_delete_queue(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "In-process bindings under development. Use Docker.",
        ))
    }
}

#[pymodule]
fn ruststack_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustStack>()?;
    Ok(())
}
