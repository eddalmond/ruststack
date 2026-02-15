# Flask Lambda Example

This directory contains an example Flask application that can run as a Lambda function using Mangum.

## Files

- `app.py` - Simple Flask API with CRUD operations
- `requirements.txt` - Python dependencies
- `deploy.sh` - Script to create deployment zip and deploy to RustStack

## Testing with RustStack

1. Start RustStack:
   ```bash
   cd /path/to/ruststack
   cargo run --release
   ```

2. Deploy the function:
   ```bash
   ./deploy.sh
   ```

3. Invoke the function:
   ```bash
   # Health check
   aws lambda invoke --endpoint-url http://localhost:4566 \
     --function-name flask-app \
     --payload '{"httpMethod":"GET","path":"/health"}' \
     response.json

   # Create item
   aws lambda invoke --endpoint-url http://localhost:4566 \
     --function-name flask-app \
     --payload '{"httpMethod":"POST","path":"/items","body":"{\"name\":\"test\"}"}' \
     response.json
   ```

## API Gateway Event Format

When invoking, use the API Gateway v1 event format:

```json
{
  "httpMethod": "GET",
  "path": "/health",
  "headers": {},
  "queryStringParameters": null,
  "body": null,
  "isBase64Encoded": false
}
```
