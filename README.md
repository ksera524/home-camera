# home-camera

Batch app for hourly RTSP recording and upload to RustFS (S3-compatible storage).

## What it does

- Connects to an RTSP camera URL from `RTSP_URL`
- Records stream for `RECORD_SECONDS` (default: 3600) using ffmpeg
- Creates MP4 file locally
- Uploads MP4 to RustFS using S3 API
- Uses object key format: `camera/YYYY/MM/DD/HH.mp4` (JST)

## Required environment variables

- `RTSP_URL`
- `RUSTFS_S3_ENDPOINT`
- `RUSTFS_S3_REGION`
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`

## Optional environment variables

- `S3_BUCKET` (default: `home-camera-recordings`)
- `CAMERA_ID` (default: `camera`)
- `RECORD_SECONDS` (default: `3600`)
- `FFMPEG_PRESET` (default: `veryfast`)
- `FFMPEG_CRF` (default: `23`, range: `0..=51`)

## Local run

```bash
cargo run --release
```

## Tests (PBT-first)

```bash
cargo test
```

Property-based tests cover:

- S3 key generation shape and camera id sanitization
- Config parsing constraints
- Retry backoff monotonicity and upper cap

## Kubernetes

- CronJob manifest: `k8s/cronjob.yaml`
- ConfigMap reference: `k8s/configmap-ref.yaml`
- Secret template: `k8s/secret.example.yaml`

Apply in order:

```bash
kubectl apply -f k8s/configmap-ref.yaml
kubectl apply -f k8s/secret.example.yaml
kubectl apply -f k8s/cronjob.yaml
```
