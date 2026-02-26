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
//! rs.s3_create_bucket("my-bucket")
//! rs.s3_put_object("my-bucket", "key.txt", "Hello World")
//! content = rs.s3_get_object("my-bucket", "key.txt")
//! ```

#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;
use ruststack_s3::storage::{EphemeralStorage, ObjectStorage};
use ruststack_sqs::SqsStorage;
use std::sync::Arc;

#[pyclass]
pub struct RustStack {
    s3: Arc<EphemeralStorage>,
    sqs: Arc<SqsStorage>,
}

#[pymethods]
impl RustStack {
    #[new]
    fn new() -> Self {
        Self {
            s3: Arc::new(EphemeralStorage::new()),
            sqs: Arc::new(SqsStorage::new()),
        }
    }

    // ============ DynamoDB ============
    // Note: Use Docker method for DynamoDB - complex API

    fn ddb_create_table(&self, _name: &str, _key_attr: &str, _key_type: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "DynamoDB: use Docker method",
        ))
    }

    fn ddb_put_item(&self, _table_name: &str, _key: &str, _value: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "DynamoDB: use Docker method",
        ))
    }

    fn ddb_get_item(&self, _table_name: &str, _key: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "DynamoDB: use Docker method",
        ))
    }

    fn ddb_list_tables(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "DynamoDB: use Docker method",
        ))
    }

    fn ddb_delete_table(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "DynamoDB: use Docker method",
        ))
    }

    // ============ S3 ============

    fn s3_create_bucket(&self, name: &str) -> PyResult<String> {
        let s3 = self.s3.clone();
        futures::executor::block_on(s3.create_bucket(name))
            .map(|_| name.to_string())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn s3_put_object(&self, bucket: &str, key: &str, value: &str) -> PyResult<()> {
        use ruststack_s3::storage::ObjectMetadata;
        let s3 = self.s3.clone();
        let data = bytes::Bytes::from(value.to_string());
        futures::executor::block_on(s3.put_object(bucket, key, data, ObjectMetadata::default()))
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn s3_get_object(&self, bucket: &str, key: &str) -> PyResult<Option<String>> {
        let s3 = self.s3.clone();
        match futures::executor::block_on(s3.get_object(bucket, key, None)) {
            Ok(v) => Ok(Some(String::from_utf8_lossy(&v.data).to_string())),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ObjectNotFound") || err_str.contains("not found") {
                    Ok(None)
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(err_str))
                }
            }
        }
    }

    fn s3_list_objects(&self, bucket: &str) -> PyResult<Vec<String>> {
        let s3 = self.s3.clone();
        match futures::executor::block_on(s3.list_objects(bucket, None, None, None, 1000)) {
            Ok(result) => Ok(result.objects.into_iter().map(|o| o.key).collect()),
            Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
        }
    }

    fn s3_delete_object(&self, bucket: &str, key: &str) -> PyResult<()> {
        let s3 = self.s3.clone();
        futures::executor::block_on(s3.delete_object(bucket, key, None))
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn s3_delete_bucket(&self, name: &str) -> PyResult<()> {
        let s3 = self.s3.clone();
        futures::executor::block_on(s3.delete_bucket(name))
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn s3_bucket_exists(&self, bucket: &str) -> PyResult<bool> {
        let s3 = self.s3.clone();
        Ok(futures::executor::block_on(s3.bucket_exists(bucket)))
    }

    fn s3_list_buckets(&self) -> PyResult<Vec<String>> {
        let s3 = self.s3.clone();
        futures::executor::block_on(s3.list_buckets())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    // ============ Secrets Manager ============

    fn secrets_create_secret(&self, _name: &str, _value: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Secrets Manager: use Docker method",
        ))
    }

    fn secrets_get_secret_value(&self, _name: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Secrets Manager: use Docker method",
        ))
    }

    fn secrets_put_secret_value(&self, _name: &str, _value: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Secrets Manager: use Docker method",
        ))
    }

    fn secrets_delete_secret(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Secrets Manager: use Docker method",
        ))
    }

    fn secrets_list_secrets(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Secrets Manager: use Docker method",
        ))
    }

    // ============ Firehose ============

    fn firehose_create_delivery_stream(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Firehose: use Docker method",
        ))
    }

    fn firehose_put_record(&self, _stream_name: &str, _data: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Firehose: use Docker method",
        ))
    }

    fn firehose_put_record_batch(
        &self,
        _stream_name: &str,
        _records: Vec<String>,
    ) -> PyResult<usize> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Firehose: use Docker method",
        ))
    }

    fn firehose_delete_delivery_stream(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Firehose: use Docker method",
        ))
    }

    // ============ IAM ============

    fn iam_create_role(&self, _name: &str, _policy: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "IAM: use Docker method",
        ))
    }

    fn iam_get_role(&self, _name: &str) -> PyResult<Option<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "IAM: use Docker method",
        ))
    }

    fn iam_list_roles(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "IAM: use Docker method",
        ))
    }

    fn iam_delete_role(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "IAM: use Docker method",
        ))
    }

    // ============ SNS ============

    fn sns_create_topic(&self, _name: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SNS: use Docker method",
        ))
    }

    fn sns_publish(&self, _topic: &str, _message: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SNS: use Docker method",
        ))
    }

    fn sns_subscribe(&self, _topic: &str, _protocol: &str, _endpoint: &str) -> PyResult<String> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SNS: use Docker method",
        ))
    }

    fn sns_list_topics(&self) -> PyResult<Vec<String>> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SNS: use Docker method",
        ))
    }

    fn sns_delete_topic(&self, _name: &str) -> PyResult<()> {
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "SNS: use Docker method",
        ))
    }

    // ============ SQS ============

    fn sqs_create_queue(&self, name: &str) -> PyResult<String> {
        self.sqs
            .create_queue(name)
            .map(|q| q.url)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sqs_send_message(&self, queue: &str, body: &str) -> PyResult<String> {
        self.sqs
            .send_message(queue, body.to_string())
            .map(|m| m.message_id)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sqs_receive_message(&self, queue: &str, max_messages: i32) -> PyResult<Vec<String>> {
        self.sqs
            .receive_message(queue, max_messages)
            .map(|msgs| msgs.into_iter().map(|m| m.body).collect())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sqs_delete_message(&self, queue: &str, receipt_handle: &str) -> PyResult<()> {
        self.sqs
            .delete_message(queue, receipt_handle)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sqs_delete_queue(&self, name: &str) -> PyResult<()> {
        self.sqs
            .delete_queue(name)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sqs_get_queue_url(&self, name: &str) -> PyResult<String> {
        self.sqs
            .get_queue(name)
            .map(|q| q.url)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    #[pyo3(signature = (prefix = None,))]
    fn sqs_list_queues(&self, prefix: Option<String>) -> PyResult<Vec<String>> {
        Ok(self.sqs.list_queues(prefix.as_deref()))
    }

    fn sqs_purge_queue(&self, _name: &str) -> PyResult<()> {
        // Purge is not implemented in storage - just return ok
        Ok(())
    }
}

#[pymodule]
fn ruststack_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustStack>()?;
    Ok(())
}
