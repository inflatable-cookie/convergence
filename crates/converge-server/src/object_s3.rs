//! S3-compatible `ObjectStore` (arch doc 14 §2, feature `backend-s3`).
//! Keys: `objects/<kind>/<hash>` (S3 needs no fanout). Verify-on-read and
//! idempotent puts carry over unchanged.

use anyhow::{Context, Result, anyhow};
use s3::Bucket;

use converge_model::ObjectId;

use crate::storage::{ObjectKind, ObjectStore};

pub struct S3ObjectStore {
    bucket: Box<Bucket>,
}

impl S3ObjectStore {
    /// `endpoint` like `http://127.0.0.1:9000` (MinIO-friendly);
    /// credentials from the standard AWS env variables.
    pub fn connect(endpoint: &str, bucket: &str, region: &str) -> Result<Self> {
        let region = s3::Region::Custom {
            region: region.to_string(),
            endpoint: endpoint.trim_end_matches('/').to_string(),
        };
        let credentials =
            s3::creds::Credentials::default().context("S3 credentials from environment")?;
        let bucket = Bucket::new(bucket, region, credentials)
            .context("open bucket")?
            .with_path_style();
        Ok(Self { bucket })
    }

    fn key(kind: ObjectKind, id: &ObjectId) -> String {
        format!("objects/{}/{}", kind.dir(), id.as_str())
    }

    fn hash(bytes: &[u8]) -> ObjectId {
        ObjectId(blake3::hash(bytes).to_hex().to_string())
    }
}

impl ObjectStore for S3ObjectStore {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        let id = Self::hash(bytes);
        self.put_bytes(kind, &id, bytes)?;
        Ok(id)
    }

    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = Self::hash(bytes);
        if actual != *id {
            return Err(anyhow!(
                "{} hash mismatch (expected {}, got {})",
                kind.dir(),
                id.as_str(),
                actual.as_str()
            ));
        }
        // Idempotent put: content addressing makes overwrite harmless, but
        // skip the write when the object already exists.
        if self.has(kind, id) {
            return Ok(());
        }
        let response = self
            .bucket
            .put_object(Self::key(kind, id), bytes)
            .context("s3 put")?;
        if response.status_code() >= 300 {
            return Err(anyhow!("s3 put failed: {}", response.status_code()));
        }
        Ok(())
    }

    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        let response = self
            .bucket
            .get_object(Self::key(kind, id))
            .context("s3 get")?;
        if response.status_code() >= 300 {
            return Err(anyhow!(
                "s3 get {} {}: {}",
                kind.dir(),
                id.as_str(),
                response.status_code()
            ));
        }
        let bytes = response.to_vec();
        let actual = Self::hash(&bytes);
        if actual != *id {
            return Err(anyhow!(
                "{} integrity check failed for {}",
                kind.dir(),
                id.as_str()
            ));
        }
        Ok(bytes)
    }

    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.bucket
            .head_object(Self::key(kind, id))
            .map(|(_, code)| code == 200)
            .unwrap_or(false)
    }

    fn list(&self, kind: ObjectKind) -> Result<Vec<(ObjectId, u64, std::time::SystemTime)>> {
        let prefix = format!("objects/{}/", kind.dir());
        let pages = self.bucket.list(prefix.clone(), None).context("s3 list")?;
        let mut out = Vec::new();
        for page in pages {
            for object in page.contents {
                let Some(name) = object.key.strip_prefix(&prefix) else {
                    continue;
                };
                let mtime = time::OffsetDateTime::parse(
                    &object.last_modified,
                    &time::format_description::well_known::Rfc3339,
                )
                .map(std::time::SystemTime::from)
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((ObjectId(name.to_string()), object.size, mtime));
            }
        }
        Ok(out)
    }

    fn delete(&self, kind: ObjectKind, id: &ObjectId) -> Result<()> {
        self.bucket
            .delete_object(Self::key(kind, id))
            .context("s3 delete")?;
        Ok(())
    }
}
