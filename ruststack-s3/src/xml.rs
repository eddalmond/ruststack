//! XML formatting utilities for S3 responses

use crate::storage::ListObjectsResult;

/// Generate request ID (simplified)
fn request_id() -> String {
    uuid::Uuid::new_v4()
        .to_string()
        .replace("-", "")
        .to_uppercase()
}

/// Format an S3 error response as XML
pub fn format_error(code: &str, message: &str, resource: &str) -> String {
    let resource_line = if !resource.is_empty() {
        format!("  <Resource>{}</Resource>\n", resource)
    } else {
        String::new()
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
  <Code>{}</Code>
  <Message>{}</Message>
{}  <RequestId>{}</RequestId>
</Error>"#,
        code,
        message,
        resource_line,
        request_id()
    )
}

/// Format ListBuckets response
pub fn format_list_buckets(buckets: &[String]) -> String {
    let bucket_entries: String = buckets
        .iter()
        .map(|name| {
            format!(
                r#"    <Bucket>
      <Name>{}</Name>
      <CreationDate>2024-01-01T00:00:00.000Z</CreationDate>
    </Bucket>"#,
                name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListAllMyBucketsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Owner>
    <ID>000000000000</ID>
    <DisplayName>ruststack</DisplayName>
  </Owner>
  <Buckets>
{}
  </Buckets>
</ListAllMyBucketsResult>"#,
        bucket_entries
    )
}

/// Format ListObjectsV2 response
pub fn format_list_objects(
    bucket: &str,
    prefix: &Option<String>,
    delimiter: &Option<String>,
    result: &ListObjectsResult,
) -> String {
    let contents: String = result
        .objects
        .iter()
        .map(|obj| {
            format!(
                r#"  <Contents>
    <Key>{}</Key>
    <LastModified>{}</LastModified>
    <ETag>{}</ETag>
    <Size>{}</Size>
    <StorageClass>{}</StorageClass>
  </Contents>"#,
                xml_escape(&obj.key),
                obj.last_modified.format("%Y-%m-%dT%H:%M:%S.000Z"),
                xml_escape(&obj.etag),
                obj.size,
                obj.storage_class
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let common_prefixes: String = result
        .common_prefixes
        .iter()
        .map(|p| {
            format!(
                r#"  <CommonPrefixes>
    <Prefix>{}</Prefix>
  </CommonPrefixes>"#,
                xml_escape(p)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prefix_element = match prefix {
        Some(p) => format!("  <Prefix>{}</Prefix>", xml_escape(p)),
        None => "  <Prefix/>".to_string(),
    };

    let delimiter_element = match delimiter {
        Some(d) => format!("  <Delimiter>{}</Delimiter>", xml_escape(d)),
        None => String::new(),
    };

    let continuation_element = match &result.next_continuation_token {
        Some(token) => format!("  <NextContinuationToken>{}</NextContinuationToken>", token),
        None => String::new(),
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>{}</Name>
{}
{}
  <MaxKeys>1000</MaxKeys>
  <IsTruncated>{}</IsTruncated>
{}
{}
{}
</ListBucketResult>"#,
        bucket,
        prefix_element,
        delimiter_element,
        result.is_truncated,
        continuation_element,
        contents,
        common_prefixes
    )
}

/// Format a CopyObject or PutObject result
pub fn format_object_result(etag: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CopyObjectResult>
  <ETag>{}</ETag>
  <LastModified>{}</LastModified>
</CopyObjectResult>"#,
        etag,
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z")
    )
}

/// Format CreateMultipartUpload response
pub fn format_create_multipart_upload(bucket: &str, key: &str, upload_id: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CreateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Bucket>{}</Bucket>
  <Key>{}</Key>
  <UploadId>{}</UploadId>
</CreateMultipartUploadResult>"#,
        xml_escape(bucket),
        xml_escape(key),
        upload_id
    )
}

/// Format CompleteMultipartUpload response
pub fn format_complete_multipart_upload(bucket: &str, key: &str, etag: &str, location: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Location>{}</Location>
  <Bucket>{}</Bucket>
  <Key>{}</Key>
  <ETag>{}</ETag>
</CompleteMultipartUploadResult>"#,
        xml_escape(location),
        xml_escape(bucket),
        xml_escape(key),
        etag
    )
}

/// Format AbortMultipartUpload response
pub fn format_abort_multipart_upload() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<AbortMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
</AbortMultipartUploadResult>"#.to_string()
}

/// Info about a multipart upload (for listing)
#[derive(Debug)]
pub struct MultipartUploadInfo {
    pub key: String,
    pub upload_id: String,
    pub initiated: chrono::DateTime<chrono::Utc>,
}

/// Info about a part (for listing)
#[derive(Debug)]
pub struct PartInfo {
    pub part_number: i32,
    pub etag: String,
    pub size: u64,
}

/// Format ListMultipartUploads response
pub fn format_list_multipart_uploads(
    bucket: &str,
    uploads: &[MultipartUploadInfo],
) -> String {
    let upload_entries: String = uploads
        .iter()
        .map(|u| {
            format!(
                r#"    <Upload>
      <Key>{}</Key>
      <UploadId>{}</UploadId>
      <Initiator>
        <ID>000000000000</ID>
        <DisplayName>ruststack</DisplayName>
      </Initiator>
      <Owner>
        <ID>000000000000</ID>
        <DisplayName>ruststack</DisplayName>
      </Owner>
      <StorageClass>STANDARD</StorageClass>
      <Initiated>{}</Initiated>
    </Upload>"#,
                xml_escape(&u.key),
                u.upload_id,
                u.initiated.format("%Y-%m-%dT%H:%M:%S.000Z")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListMultipartUploadsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Bucket>{}</Bucket>
  <KeyMarker/>
  <UploadIdMarker/>
  {}
</ListMultipartUploadsResult>"#,
        bucket,
        if upload_entries.is_empty() {
            String::new()
        } else {
            format!("  <Uploads>\n{}  </Uploads>", upload_entries)
        }
    )
}

/// Format ListParts response
pub fn format_list_parts(
    bucket: &str,
    key: &str,
    upload_id: &str,
    parts: &[PartInfo],
) -> String {
    let part_entries: String = parts
        .iter()
        .map(|p| {
            format!(
                r#"    <Part>
      <PartNumber>{}</PartNumber>
      <ETag>{}</ETag>
      <Size>{}</Size>
    </Part>"#,
                p.part_number, p.etag, p.size
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ListPartsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Bucket>{}</Bucket>
  <Key>{}</Key>
  <UploadId>{}</UploadId>
  <StorageClass>STANDARD</StorageClass>
  <IsTruncated>false</IsTruncated>
{}
</ListPartsResult>"#,
        bucket,
        xml_escape(key),
        upload_id,
        if part_entries.is_empty() {
            String::new()
        } else {
            format!("  <Parts>\n{}  </Parts>", part_entries)
        }
    )
}

/// XML escape special characters
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_error() {
        let xml = format_error("NoSuchBucket", "The bucket does not exist", "my-bucket");
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Resource>my-bucket</Resource>"));
    }

    #[test]
    fn test_format_list_buckets() {
        let buckets = vec!["bucket1".to_string(), "bucket2".to_string()];
        let xml = format_list_buckets(&buckets);
        assert!(xml.contains("<Name>bucket1</Name>"));
        assert!(xml.contains("<Name>bucket2</Name>"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("test&value"), "test&amp;value");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
    }
}
