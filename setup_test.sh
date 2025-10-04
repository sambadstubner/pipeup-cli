#!/bin/bash

# CLI Integration Test Setup Script
set -e

echo "🚀 Setting up CLI integration test environment..."

# Backend URL
API_URL="http://localhost:8000"

# Test user credentials
TEST_EMAIL="test@example.com"
TEST_USERNAME="testuser"
TEST_PASSWORD="testpassword123"

echo "📧 Creating test user: $TEST_EMAIL"

# Register test user
REGISTER_RESPONSE=$(curl -s -X POST "$API_URL/auth/register" \
  -H "Content-Type: application/json" \
  -d "{
    \"email\": \"$TEST_EMAIL\",
    \"username\": \"$TEST_USERNAME\",
    \"password\": \"$TEST_PASSWORD\"
  }")

echo "Register response: $REGISTER_RESPONSE"

# Login to get JWT token
echo "🔐 Logging in to get JWT token..."

LOGIN_RESPONSE=$(curl -s -X POST "$API_URL/auth/login" \
  -H "Content-Type: application/json" \
  -d "{
    \"email\": \"$TEST_EMAIL\",
    \"password\": \"$TEST_PASSWORD\"
  }")

echo "Login response: $LOGIN_RESPONSE"

# Extract JWT token
JWT_TOKEN=$(echo $LOGIN_RESPONSE | python3 -c "import sys, json; print(json.load(sys.stdin)['access_token'])" 2>/dev/null || echo "")

if [ -z "$JWT_TOKEN" ]; then
  echo "❌ Failed to get JWT token"
  echo "Login response: $LOGIN_RESPONSE"
  exit 1
fi

echo "✅ JWT token obtained"

# Create API token for CLI
echo "🔑 Creating API token for CLI..."

API_TOKEN_RESPONSE=$(curl -s -X POST "$API_URL/auth/api-tokens" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d "{
    \"name\": \"CLI Test Token\"
  }")

echo "API token response: $API_TOKEN_RESPONSE"

# Extract API token
API_TOKEN=$(echo $API_TOKEN_RESPONSE | python3 -c "import sys, json; print(json.load(sys.stdin)['raw_token'])" 2>/dev/null || echo "")

if [ -z "$API_TOKEN" ]; then
  echo "❌ Failed to get API token"
  echo "API token response: $API_TOKEN_RESPONSE"
  exit 1
fi

echo "✅ API token created: $API_TOKEN"

# Export environment variable
export PIPEUP_TOKEN="$API_TOKEN"

echo ""
echo "🎉 Setup complete! Use this command to set the environment variable:"
echo "export PIPEUP_TOKEN=\"$API_TOKEN\""
echo ""
echo "💡 Test the CLI with:"
echo "echo 'Hello, Pipeup!' | ./target/debug/pipeup --name 'Test Stream'"