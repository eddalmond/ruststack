#!/bin/bash
# Deploy Flask app to RustStack Lambda

set -e

ENDPOINT_URL="${RUSTSTACK_ENDPOINT:-http://localhost:4566}"
FUNCTION_NAME="flask-app"

echo "Creating deployment package..."
rm -rf /tmp/flask-lambda-deploy
mkdir -p /tmp/flask-lambda-deploy

# Copy application code
cp app.py /tmp/flask-lambda-deploy/

# Install dependencies
pip install -r requirements.txt -t /tmp/flask-lambda-deploy/ --quiet

# Create zip
cd /tmp/flask-lambda-deploy
zip -r /tmp/flask-lambda.zip . -q

echo "Deploying to RustStack at $ENDPOINT_URL..."

# Check if function exists
if aws lambda get-function \
    --endpoint-url "$ENDPOINT_URL" \
    --function-name "$FUNCTION_NAME" \
    --no-cli-pager 2>/dev/null; then

    echo "Updating existing function..."
    aws lambda update-function-code \
        --endpoint-url "$ENDPOINT_URL" \
        --function-name "$FUNCTION_NAME" \
        --zip-file fileb:///tmp/flask-lambda.zip \
        --no-cli-pager
else
    echo "Creating new function..."
    aws lambda create-function \
        --endpoint-url "$ENDPOINT_URL" \
        --function-name "$FUNCTION_NAME" \
        --runtime python3.12 \
        --role "arn:aws:iam::000000000000:role/lambda-role" \
        --handler "app.handler" \
        --zip-file fileb:///tmp/flask-lambda.zip \
        --timeout 30 \
        --memory-size 256 \
        --no-cli-pager
fi

echo "Done! Test with:"
echo "aws lambda invoke --endpoint-url $ENDPOINT_URL --function-name $FUNCTION_NAME --payload '{\"httpMethod\":\"GET\",\"path\":\"/health\"}' response.json && cat response.json"
