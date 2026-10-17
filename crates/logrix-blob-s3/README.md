# logrix-blob-s3

AWS SDK implementation of the core `BlobPort`: store, retrieve, delete, and check
byte buffers in S3, with optional explicit Zstandard compression helpers. Custom
endpoints/path-style addressing support S3-compatible services such as MinIO,
LocalStack, or R2; compatibility depends on the service's S3 API behavior.

## Prerequisites

Use the repository's Rust workspace and lockfile, Tokio, and a pre-created
bucket. Configure region and credentials through the AWS SDK provider chain or
provide your own SDK client. Grant object get/head, put, and delete permissions
for the operations used. The crate does not create buckets, configure lifecycle
or retention rules, or provision a local object store.

## Minimal example

With `logrix-blob-s3` and Tokio, this application explicitly reads its bucket
name and round-trips caller-supplied bytes:

```rust
use logrix_blob_s3::{S3BlobStore, S3Config, DEFAULT_ZSTD_LEVEL};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = S3Config::builder(std::env::var("S3_BUCKET")?)
        .region("us-east-1")
        .key_prefix("examples")
        .build()
        .await?;
    let store = S3BlobStore::new(config);
    let data = b"caller-supplied archive bytes";
    store
        .put_compressed("sample.bin.zst", data, DEFAULT_ZSTD_LEVEL)
        .await?;
    let restored = store.get_decompressed("sample.bin.zst").await?;
    assert_eq!(restored.as_deref(), Some(data.as_slice()));
    Ok(())
}
```

Import `logrix_core::BlobPort` to call the trait methods `put`, `get`, `delete`,
or `exists` directly. For a local endpoint, add
`.endpoint_url("http://localhost:9000")` and configure credentials appropriate
for that service.

## Public configuration APIs and defaults

`S3Config` has public fields `client: aws_sdk_s3::Client`, `bucket: String`,
and `key_prefix: Option<String>`. `S3Config::new(client, bucket, prefix)` uses
the supplied client as-is. `S3Config::builder(bucket)` returns
`S3ClientOptionsBuilder`, also constructible with `S3ClientOptionsBuilder::new`.

| Builder method | Default | Meaning |
|---|---|---|
| Required `bucket` argument | No crate default | Bucket for every operation. Empty/invalid names are not rejected by this builder. |
| `.key_prefix(prefix)` | `None` | Prefix applied by `resolve_key`. |
| `.region(region)` | AWS SDK provider chain | Explicit region override; no crate-level `us-east-1` fallback. |
| `.endpoint_url(url)` | No explicit override | Custom AWS SDK endpoint. |
| `.force_path_style(bool)` | Automatically enabled when an endpoint override is supplied | Addressing option. Explicit false skips the adapter's automatic enabling. |

`build().await` loads SDK config and constructs a client without validating
bucket access or performing an object request. The builder has no validation
method, operation-timeout fields, or retry-policy fields. For detailed SDK
settings, construct a client and pass it to `S3Config::new`.

`S3Config::from_env(bucket, key_prefix, endpoint_override).await` takes all
three Logrix settings as arguments. It loads AWS SDK config and forces
path-style addressing when its endpoint argument is `Some`. It has no separate
path-style override. `S3BlobStore::new(config)` constructs the store, and
`config()` exposes an immutable reference to its configuration.

`resolve_key(key)` leaves a key unchanged for `None` or an empty prefix. For
other prefixes, it removes trailing `/` characters and concatenates
`prefix + "/" + key`. It does not normalize a leading slash in the key,
validate path components, or add chain/block-specific segments or extensions.

### Environment, CLI, and manifest ownership

`from_env` and `build` use AWS SDK region/credential configuration (for example
`AWS_REGION`, `AWS_PROFILE`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and
`AWS_SESSION_TOKEN`). This crate does not itself parse `S3_BUCKET`, `S3_PREFIX`,
or `S3_ENDPOINT`; the CLI owns these mappings:

| Variable | CLI flag | CLI default |
|---|---|---|
| `S3_BUCKET` | `--s3-bucket` | None; processor S3 setup is optional |
| `S3_PREFIX` | `--s3-prefix` | None |
| `S3_ENDPOINT` | `--s3-endpoint` | None |

The example above reads `S3_BUCKET` explicitly. This crate does not load
`logrix.yaml`; any SDK/manifest archive settings must be translated by their
own tooling into constructor or CLI inputs.

## Storage semantics and fixed limits

- `put` uploads the supplied bytes to one resolved key, with no serialization or
  compression added. `get` collects the entire body into `Vec<u8>`. `exists`
  overrides the core default with `HeadObject`, avoiding a body download.
- `put_compressed(key, data, level)` compresses first and calls `put`.
  `get_decompressed(key)` calls `get` and then decodes Zstandard. Neither infers
  compression from the file extension nor adds a content-encoding marker.
  Match the chosen writer/reader explicitly.
- `compress_zstd` and `decompress_zstd` are synchronous byte-buffer helpers.
  `DEFAULT_ZSTD_LEVEL` is 3; any level less than or equal to zero selects 3,
  while positive levels are passed to Zstandard. There is no promised 80%
  reduction: output size depends on input and can exceed the original size.
- Uploads copy the input; downloads and (de)compression buffer whole objects.
  There is no multipart-upload API, streaming port, object-size preflight, or
  decompressed-size cap in this crate. Size data to the application's memory
  budget and the object service's limits.
- Reusing a key follows the service's normal overwrite/versioning behavior;
  the adapter adds no conditional-write, deduplication, or checkpoint transaction.

The adapter has no background archiver, block/receipt fetcher, entity-snapshot
generator, or automatic restore/backfill loop. The CLI's `index_envelope` path
separately serializes/compresses a block envelope and attempts an upload after
the database commit, ignoring archive errors. Its range-log indexing path does
not perform that upload. Configuring this crate alone does not archive all
blocks, receipts, or entity state, or make an archive part of checkpoint success.

## Errors

Store methods return `LogrixResult` with `ErrorSource::BlobStore`. SDK request
and body-read failures are generally `Transient`, including authorization or
validation failures; the classification does not imply automatic retries here.
`get` returns `Ok(None)` only for the SDK's `NoSuchKey` service error. `exists`
returns false for modeled `NotFound` or raw HTTP 404; other failures are errors.

Compression failures in `put_compressed` are `Permanent`, and decompression
failures in `get_decompressed` are `Integrity`. The standalone compression
functions return `std::io::Error`. Missing compressed objects still return
`Ok(None)`, not a decompression error.

## Tests and coverage

From the repository root, with dependencies cached; the environment settings
make region selection explicit and disable EC2 metadata discovery:

```sh
AWS_REGION=us-east-1 AWS_EC2_METADATA_DISABLED=true cargo test -p logrix-blob-s3 --offline --locked
```

Unit/integration tests cover Zstandard round trips, repetitive input becoming
smaller, empty/corrupt data, key prefixes, and client/config construction with
endpoint overrides. They do not upload/download objects or verify service
compatibility, missing-object responses, permissions, overwrites, large-object
memory use, or an end-to-end CLI archive. No compression percentage is tested.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core BlobPort](../logrix-core/README.md)
- [CLI integration](../logrix-cli/README.md) and [PostgreSQL checkpoints](../logrix-store-postgres/README.md)
- [Deployment manifests](../../deploy/)
