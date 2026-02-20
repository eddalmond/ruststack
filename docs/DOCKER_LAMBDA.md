# Docker-Based Lambda Execution

## Overview

RustStack supports two Lambda execution modes:

1. **Subprocess mode** (default) - Fast, runs Python directly on host
2. **Docker mode** - Isolated, runs lambdas in containers

## Why Docker Mode?

| Aspect | Subprocess | Docker |
|--------|------------|--------|
| **Speed** | ~10-50ms cold start | ~500ms-2s cold start |
| **Isolation** | None - shares host environment | Full container isolation |
| **Dependencies** | Must be installed on host | Bundled in container |
| **Runtimes** | Python only (host version) | Any runtime with image |
| **Security** | Low - full host access | High - sandboxed |

## When to Use Each

**Use subprocess (default):**
- Fast iteration during development
- Simple functions with no deps
- Host has correct Python version
- Performance matters

**Use Docker:**
- Functions need specific dependencies
- Testing production-like isolation
- Multiple Python versions needed
- Security matters (untrusted code)

## Usage

```bash
# Default (subprocess)
ruststack

# Force Docker mode for all lambdas
ruststack --lambda-executor docker

# Hybrid: use Docker only when needed
ruststack --lambda-executor auto
```

In `auto` mode, RustStack uses Docker when:
- Function has `Layers` configured
- Function specifies a custom image
- `--force-docker` is set in function tags

## Lambda Layers

When using Docker mode, you can specify Lambda layers to include additional dependencies:

```python
import boto3

lambda_client = boto3.client("lambda", endpoint_url="http://localhost:4566", ...)

lambda_client.create_function(
    FunctionName='my-function',
    Runtime='python3.12',
    Handler='handler.main',
    Role='arn:aws:iam::123456789012:role/lambda-role',
    Code={
        'ZipFile': open('function.zip', 'rb').read(),
    },
    Layers=['/path/to/numpy-layer.zip', '/path/to/shared-libs.zip']
)
```

### How Layers Work

1. Layer ZIP files are mounted into the container at `/tmp/layerN.zip`
2. Each layer is extracted to `/opt/` inside the container
3. Python automatically adds `/opt/python/lib/python3.X/site-packages/` to `PYTHONPATH`

### Layer Structure

For Python layers, structure your ZIP like:
```
layer.zip
├── python/
│   └── lib/
│       └── python3.12/
│           └── site-packages/
│               ├── numpy/
│               └── pandas/
└── (other files go to /opt/)
```

### Differences from AWS Lambda

- AWS extracts layers to `/opt/python/lib/python3.12/site-packages/`
- RustStack extracts to `/opt/` - you may need to adjust PYTHONPATH in your handler

## S3 Code Deployment

You can deploy Lambda functions with code stored in S3 (useful for large functions):

```python
# Upload code to S3 first
s3 = boto3.client("s3", endpoint_url="http://localhost:4566", ...)
s3.put_object(Bucket='my-bucket', Key='function.zip', Body=open('function.zip', 'rb').read())

# Create function from S3
lambda_client.create_function(
    FunctionName='my-function',
    Runtime='python3.12',
    Handler='handler.main',
    Role='arn:aws:iam::123456789012:role/lambda-role',
    Code={
        'S3Bucket': 'my-bucket',
        'S3Key': 'function.zip',
        # Optional: 'S3ObjectVersion': 'version-id'
    }
)
```

## Docker Images

RustStack uses AWS Lambda-compatible base images:

| Runtime | Image |
|---------|-------|
| python3.9 | `public.ecr.aws/lambda/python:3.9` |
| python3.10 | `public.ecr.aws/lambda/python:3.10` |
| python3.11 | `public.ecr.aws/lambda/python:3.11` |
| python3.12 | `public.ecr.aws/lambda/python:3.12` |
| python3.13 | `public.ecr.aws/lambda/python:3.13` |
| nodejs18.x | `public.ecr.aws/lambda/nodejs:18` |
| nodejs20.x | `public.ecr.aws/lambda/nodejs:20` |

## Container Lifecycle

```
Invoke Request
     │
     ▼
┌────────────────────────────────┐
│ Is warm container available?   │
│   │                            │
│   ├─► YES: Reuse container     │
│   │                            │
│   └─► NO: Start new container  │
│        (cold start)            │
└────────────────────────────────┘
     │
     ▼
Container executes handler
     │
     ▼
Response returned
     │
     ▼
Container stays warm (configurable TTL)
```

### Warm Container Pool

Containers are kept warm for reuse (default: 5 minutes idle).

```bash
# Configure warm pool
ruststack --lambda-container-ttl 300  # seconds
ruststack --lambda-max-containers 10   # max concurrent
```

## Performance Comparison

Benchmark on M1 Mac:

| Scenario | Subprocess | Docker (cold) | Docker (warm) |
|----------|------------|---------------|---------------|
| Simple return | 15ms | 1.2s | 45ms |
| With boto3 | 180ms | 1.5s | 180ms |
| Complex handler | 250ms | 1.8s | 260ms |

## Requirements

Docker mode requires:
- Docker daemon running
- Pull access to `public.ecr.aws/lambda/*` images
- Network access for containers to reach ruststack

## Network Configuration

Containers need to reach RustStack for S3/DynamoDB access:

```bash
# On Linux
ruststack --lambda-network host

# On Mac/Windows (Docker Desktop)
ruststack --lambda-network bridge  # uses host.docker.internal
```

## Troubleshooting

### Container can't reach RustStack
```bash
# Check Docker network
docker network inspect bridge

# On Mac, ensure host.docker.internal resolves
docker run --rm alpine ping host.docker.internal
```

### Slow cold starts
```bash
# Pre-pull images
docker pull public.ecr.aws/lambda/python:3.12

# Or increase warm pool
ruststack --lambda-max-containers 20 --lambda-container-ttl 600
```

### Permission denied
```bash
# Add user to docker group (Linux)
sudo usermod -aG docker $USER
newgrp docker
```
