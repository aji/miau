#!/bin/sh

S3BLOB='s3://miau-build-archives/miau-latest.tar.bz2'
ARCHIVE='/tmp/miau-latest.tar.bz2'

fail() {
  echo "$1 failed, stopping." >&2
  exit 1
}

echo "Running build" >&2
cargo build --release   || fail "Tests"

echo "Running tests" >&2
cargo test --release  || fail "Build"

echo "Build finished, preparing artifacts" >&2
cp scripts/appspec-staging.yml appspec.yml || fail "Copy appspec"
tar cjvf "$ARCHIVE" $(cat scripts/manifest.txt) || fail "Archive"

echo "Uploading archive to S3" >&2
aws s3 cp "$S3BLOB" "$ARCHIVE" || fail "S3 upload"
