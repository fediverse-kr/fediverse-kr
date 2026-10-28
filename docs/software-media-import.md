# Runtime software media import

The release binary accepts the operator-only import command:

```text
server --software-media-import /absolute/path/manifest.json [--dry-run]
```

When invoking the project binary directly, the equivalent form is
`fediversekr2 server --software-media-import /absolute/path/manifest.json`.
The command exits before HTTP workers start. `--dry-run` validates the complete
manifest and every asset, checks catalog names and expected revisions, and changes
no media objects or catalog rows. Normal configured database initialization still
applies pending schema migrations; run against an initialized database when a
strictly read-only schema check is required.

## Manifest

```json
{
  "version": 1,
  "entries": [
    {
      "name": "arbitrary-catalog-name",
      "revision": 12,
      "logo": {
        "file": "files/logo.png",
        "sha256": "64-lowercase-hex-characters",
        "metadata": {
          "source_url": "https://example.org/brand",
          "attribution": "Project contributors",
          "license": "CC BY 4.0",
          "license_url": "https://creativecommons.org/licenses/by/4.0/",
          "caption": "",
          "fit": "contain",
          "background": "#FFFFFF"
        }
      },
      "header": {
        "file": "files/header.png",
        "sha256": "64-lowercase-hex-characters",
        "metadata": {
          "source_url": "https://example.org/",
          "attribution": "Project contributors",
          "license": "CC BY 4.0",
          "license_url": "https://creativecommons.org/licenses/by/4.0/",
          "caption": "Official product screen",
          "fit": "cover",
          "background": "#F2F2F2"
        }
      }
    }
  ]
}
```

`logo` and `header` are optional. Omitting a role preserves the live role.
Each entry must name an existing catalog software row, use the exact current
revision, and contain at least one role. File paths are relative to the
manifest directory; absolute paths, `..`, and symlinks are rejected. Files are
hashed with SHA-256, decoded and validated before any database or ObjectStore
write. Logos are limited to 500 KiB and 4096px per edge; headers are limited to
8 MiB, 8192px per edge, and 32 megapixels. Original bytes are retained.

The import uses the configured `ObjectStore`; it does not open a media path or
serve files directly. The current deployment must therefore provide the same
configured ObjectStore required by normal runtime media serving.

Logo and header changes for one software are committed with one revision and a
null member/actor audit attribution. Existing live media is never silently
replaced: an import whose expected revision does not bind the live prestate is
rejected. Re-running a completed import is a no-op when the live bytes and
metadata match the manifest. A failed entry aborts before the next entry is
written; run `--dry-run` first for a complete package check.

## Existing storage cutover

`server --media-store-cutover --dry-run` checks the canonical `stored_files`
inventory against the configured S3 backend and the preserved local media root.
`server --media-store-cutover` conditionally copies only missing objects through
OpenDAL, verifies every byte count and SHA-256, and reads published bytes back.
It requires `FEDKR_MEDIA_BACKEND=s3`; it never treats Fs as the destination.
The source is never deleted, and existing mismatched S3 objects are never overwritten.

Stop the old writer before the final cutover. A single-replica Recreate deployment
can run this command in an init container using the same new image, database,
S3 settings and mounted lock/source directory as the application. HTTP/workers
start only after it succeeds. Repeated runs verify already-present objects and
also accept new S3-only uploads that have no local preservation copy. The offline
transaction uses a ten-minute idle allowance while copying; ordinary HTTP
transactions retain their shorter timeout.

An image-only rollback to an Fs-only release is unsafe after S3-only writes.
Stop writers and verify a complete reverse copy of the current database-referenced
objects before starting an older Fs release. Do not restore an earlier database
dump over newer member changes.

Back up the database and local objects first. Keep the original PVC and existing
R2 objects until any separately authorized cleanup; changing the media backend
is not permission to remove the preservation source. Do not automatically import
software artwork on application startup: that separate curated-data operation
still requires the explicit manifest command above.
