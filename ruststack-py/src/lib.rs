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
use ruststack_firehose::FirehoseStorage;
use ruststack_iam::IamStorage;
use ruststack_s3::storage::{EphemeralStorage, ObjectStorage};
use ruststack_secretsmanager::{SecretsManagerStorage, SecretsManagerStorageTrait};
use ruststack_sns::SnsState;
use ruststack_sqs::SqsStorage;
use std::sync::Arc;

#[pyclass]
pub struct RustStack {
    s3: Arc<EphemeralStorage>,
    sqs: Arc<SqsStorage>,
    secrets: Arc<SecretsManagerStorage>,
    firehose: Arc<FirehoseStorage>,
    iam: Arc<IamStorage>,
    sns: SnsState,
}

#[pymethods]
impl RustStack {
    #[new]
    fn new() -> Self {
        Self {
            s3: Arc::new(EphemeralStorage::new()),
            sqs: Arc::new(SqsStorage::new()),
            secrets: Arc::new(SecretsManagerStorage::new()),
            firehose: Arc::new(FirehoseStorage::new()),
            iam: Arc::new(IamStorage::new()),
            sns: SnsState::new(),
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

    fn secrets_create_secret(&self, name: &str, value: &str) -> PyResult<String> {
        use std::collections::HashMap;
        self.secrets
            .create_secret(
                name,
                None,
                None,
                Some(value.to_string()),
                None,
                HashMap::new(),
            )
            .map(|s| s.name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn secrets_get_secret_value(&self, name: &str) -> PyResult<Option<String>> {
        match self.secrets.get_secret_value(name, None, None) {
            Ok((_s, v)) => Ok(v.secret_string),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ResourceNotFound") {
                    Ok(None)
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(err_str))
                }
            }
        }
    }

    fn secrets_put_secret_value(&self, name: &str, value: &str) -> PyResult<()> {
        self.secrets
            .put_secret_value(name, Some(value.to_string()), None)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn secrets_delete_secret(&self, name: &str) -> PyResult<()> {
        self.secrets
            .delete_secret(name, true)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn secrets_list_secrets(&self) -> PyResult<Vec<String>> {
        Ok(self
            .secrets
            .list_secrets()
            .into_iter()
            .map(|s| s.name)
            .collect())
    }

    fn secrets_describe_secret(&self, name: &str) -> PyResult<Option<String>> {
        match self.secrets.describe_secret(name) {
            Ok(s) => Ok(Some(
                serde_json::json!({
                    "name": s.name,
                    "arn": s.arn,
                    "description": s.description,
                })
                .to_string(),
            )),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ResourceNotFound") {
                    Ok(None)
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(err_str))
                }
            }
        }
    }

    // ============ Firehose ============

    fn firehose_create_delivery_stream(&self, name: &str) -> PyResult<String> {
        self.firehose
            .create_delivery_stream(name, "DirectPut", None, None, None)
            .map(|s| s.delivery_stream_name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn firehose_put_record(&self, stream_name: &str, data: &str) -> PyResult<String> {
        self.firehose
            .put_record(stream_name, data.as_bytes().to_vec())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn firehose_put_record_batch(
        &self,
        stream_name: &str,
        records: Vec<String>,
    ) -> PyResult<usize> {
        let records: Vec<Vec<u8>> = records.into_iter().map(|r| r.into_bytes()).collect();
        self.firehose
            .put_record_batch(stream_name, records)
            .map(|r| r.record_ids.len())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn firehose_delete_delivery_stream(&self, name: &str) -> PyResult<()> {
        self.firehose
            .delete_delivery_stream(name)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn firehose_list_delivery_streams(&self) -> PyResult<Vec<String>> {
        Ok(self.firehose.list_delivery_streams(None))
    }

    fn firehose_describe_delivery_stream(&self, name: &str) -> PyResult<Option<String>> {
        match self.firehose.describe_delivery_stream(name) {
            Ok(s) => Ok(Some(
                serde_json::json!({
                    "name": s.delivery_stream_name,
                    "type": s.delivery_stream_type,
                })
                .to_string(),
            )),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("ResourceNotFound") {
                    Ok(None)
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(err_str))
                }
            }
        }
    }

    // ============ IAM ============

    fn iam_create_role(&self, name: &str, policy: &str) -> PyResult<String> {
        self.iam
            .create_role(name, policy, None, None)
            .map(|r| r.role_name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn iam_get_role(&self, name: &str) -> PyResult<Option<String>> {
        match self.iam.get_role(name) {
            Ok(r) => Ok(Some(
                serde_json::json!({
                    "name": r.role_name,
                    "arn": r.arn,
                })
                .to_string(),
            )),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("NoSuchEntity") || err_str.contains("not found") {
                    Ok(None)
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(err_str))
                }
            }
        }
    }

    fn iam_list_roles(&self) -> PyResult<Vec<String>> {
        Ok(self
            .iam
            .list_roles()
            .into_iter()
            .map(|r| r.role_name)
            .collect())
    }

    fn iam_delete_role(&self, name: &str) -> PyResult<()> {
        self.iam
            .delete_role(name)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn iam_create_policy(&self, name: &str, policy: &str) -> PyResult<String> {
        self.iam
            .create_policy(name, policy, None, None)
            .map(|p| p.arn)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn iam_delete_policy(&self, arn: &str) -> PyResult<()> {
        self.iam
            .delete_policy(arn)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn iam_attach_role_policy(&self, role_name: &str, policy_arn: &str) -> PyResult<()> {
        self.iam
            .attach_role_policy(role_name, policy_arn)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn iam_detach_role_policy(&self, role_name: &str, policy_arn: &str) -> PyResult<()> {
        self.iam
            .detach_role_policy(role_name, policy_arn)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    // ============ SNS ============

    fn sns_create_topic(&self, name: &str) -> PyResult<String> {
        self.sns
            .create_topic(name)
            .map(|t| t.name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sns_publish(&self, topic: &str, message: &str) -> PyResult<String> {
        self.sns
            .publish(topic, message, None)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sns_subscribe(&self, topic: &str, protocol: &str, endpoint: &str) -> PyResult<String> {
        self.sns
            .subscribe(topic, protocol, endpoint)
            .map(|s| s.arn().to_string())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sns_list_topics(&self) -> PyResult<Vec<String>> {
        Ok(self.sns.list_topics().into_iter().map(|t| t.name).collect())
    }

    fn sns_delete_topic(&self, name: &str) -> PyResult<()> {
        self.sns
            .delete_topic(name)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sns_unsubscribe(&self, subscription_arn: &str) -> PyResult<()> {
        self.sns
            .unsubscribe(subscription_arn)
            .map(|_| ())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn sns_list_subscriptions(&self, topic: &str) -> PyResult<Vec<String>> {
        self.sns
            .list_subscriptions(topic)
            .map(|subs| subs.into_iter().map(|s| s.endpoint().to_string()).collect())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
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
        Ok(())
    }
}

#[pymodule]
fn ruststack_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustStack>()?;
    Ok(())
}
