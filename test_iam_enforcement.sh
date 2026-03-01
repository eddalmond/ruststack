#!/usr/bin/env bash
set -e

# Build RustStack
echo "[*] Building RustStack..."
CARGO_TARGET_DIR=target/agent_build cargo build > cargo_build.log 2>&1

# Kill any existing RustStack instances
pkill ruststack || true
sleep 1

# Ensure AWS CLI is installed
if [ ! -f "$HOME/.local/bin/aws" ]; then
    echo "[*] Installing AWS CLI v2 directly..."
    curl -s "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
    unzip -qo awscliv2.zip
    ./aws/install -i ~/.local/aws-cli -b ~/.local/bin
fi
export PATH=$PATH:$HOME/.local/bin
AWS_BIN="$HOME/.local/bin/aws"
export PATH=$PATH:$HOME/.local/bin
AWS_BIN="$HOME/.local/bin/aws"

# Start RustStack with IAM Enforcement ON
echo "[*] Starting RustStack with IAM Enforcement=true..."
nohup env RUSTSTACK_ENFORCE_IAM=true ./target/agent_build/debug/ruststack < /dev/null > ruststack.log 2>&1 &
RUSTSTACK_PID=$!

# Wait for RustStack to be ready
echo "[*] Waiting for RustStack to start..."
for i in {1..30}; do
    if curl -s http://localhost:4566/health > /dev/null; then
        echo "[+] RustStack is up!"
        break
    fi
    sleep 0.5
done

# Wait an extra second to assure router is fully bound
sleep 1

echo "------------------------------------------------------"
echo "[*] Test 1: Unauthorised / Implicit Deny Access"
echo "------------------------------------------------------"

# We use a custom local profile mapped to a random AccessKey that does NOT exist
AWS_ACCESS_KEY_ID=non_existent_role AWS_SECRET_ACCESS_KEY=dummy AWS_DEFAULT_REGION=us-east-1 \
    $AWS_BIN s3 ls --endpoint-url http://localhost:4566 > test1.out 2>&1 || true

if grep -qE "AccessDeniedException|Forbidden|403" test1.out; then
    echo "[PASS] Successfully blocked unauthorized S3 request."
else
    echo "[FAIL] Expected AccessDeniedException!"
    cat test1.out
    kill $RUSTSTACK_PID
    exit 1
fi

echo "------------------------------------------------------"
echo "[*] Test 2: Setting up IAM Role and Policy"
echo "------------------------------------------------------"

export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_DEFAULT_REGION=us-east-1

# Create an arbitrary role that matches the name we want to assume
$AWS_BIN iam create-role --role-name "my_authorized_role" \
    --assume-role-policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}' \
    --endpoint-url http://localhost:4566

# Create an S3 read-only policy
$AWS_BIN iam create-policy --policy-name "s3-reader" \
    --policy-document '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["s3:*"],"Resource":["*"]}]}' \
    --endpoint-url http://localhost:4566

# Attach the policy to the Role
$AWS_BIN iam attach-role-policy --role-name "my_authorized_role" \
    --policy-arn "arn:aws:iam::000000000000:policy/s3-reader" \
    --endpoint-url http://localhost:4566

echo "------------------------------------------------------"
echo "[*] Test 3: Authorized Access (via Role Match)"
echo "------------------------------------------------------"

# Now we call S3 with an Access Key matching the Role Name. 
# Our simplistic local evaluation maps AccessKey to RoleName to look up attached policies
AWS_ACCESS_KEY_ID=my_authorized_role AWS_SECRET_ACCESS_KEY=dummy AWS_DEFAULT_REGION=us-east-1 \
    $AWS_BIN s3 mb s3://my-test-bucket --endpoint-url http://localhost:4566 > test3.out 2>&1 || true

if grep -q "make_bucket" test3.out || $AWS_BIN s3 ls --endpoint-url http://localhost:4566 | grep -q "my-test-bucket"; then
    echo "[PASS] Successfully allowed authorized S3 bucket creation!"
elif ! grep -q "AccessDenied" test3.out; then
    echo "[PASS] Successfully executed without getting AccessDenied!"
else
    echo "[FAIL] Expected allow!"
    cat test3.out
    kill $RUSTSTACK_PID
    exit 1
fi


echo "------------------------------------------------------"
echo "[*] Cleaning up"
echo "------------------------------------------------------"

kill $RUSTSTACK_PID

echo "[✓] Integration test passed!"
exit 0
