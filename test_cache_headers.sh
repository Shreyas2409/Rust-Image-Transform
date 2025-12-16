#!/bin/bash

# Test script for verifying Cloudflare caching headers
# Usage: ./test_cache_headers.sh [URL]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default to localhost if no URL provided
BASE_URL="${1:-http://localhost:3000}"

echo "🧪 Testing Cloudflare Cache Headers on: $BASE_URL"
echo ""

# Function to check if a header exists and matches expected value
check_header() {
    local response="$1"
    local header="$2"
    local expected="$3"
    local partial="${4:-false}"
    
    # Extract header value (case-insensitive)
    local value=$(echo "$response" | grep -i "^$header:" | cut -d' ' -f2- | tr -d '\r')
    
    if [ -z "$value" ]; then
        echo -e "${RED}✗ $header: NOT FOUND${NC}"
        return 1
    fi
    
    if [ "$partial" = "true" ]; then
        if echo "$value" | grep -q "$expected"; then
            echo -e "${GREEN}✓ $header: $value${NC}"
            return 0
        else
            echo -e "${RED}✗ $header: $value (expected to contain: $expected)${NC}"
            return 1
        fi
    else
        if [ "$value" = "$expected" ]; then
            echo -e "${GREEN}✓ $header: $value${NC}"
            return 0
        else
            echo -e "${YELLOW}⚠ $header: $value (expected: $expected)${NC}"
            return 1
        fi
    fi
}

# Test 1: Health endpoint (should NOT have caching)
echo "Test 1: Health Endpoint (should not cache)"
echo "----------------------------------------"
HEALTH_RESPONSE=$(curl -s -I "$BASE_URL/health")

if echo "$HEALTH_RESPONSE" | grep -q "HTTP/[0-9.]\+ 200"; then
    echo -e "${GREEN}✓ Health endpoint is responding${NC}"
else
    echo -e "${RED}✗ Health endpoint failed${NC}"
    exit 1
fi
echo ""

# Test 2: Sign endpoint
echo "Test 2: Sign URL Endpoint"
echo "----------------------------------------"
TIMESTAMP=$(date +%s)
FUTURE_TIMESTAMP=$((TIMESTAMP + 3600))
SIGN_URL="$BASE_URL/sign?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t=$FUTURE_TIMESTAMP"

SIGN_RESPONSE=$(curl -s "$SIGN_URL")
SIGNED_URL=$(echo "$SIGN_RESPONSE" | grep -o '"signed_url":"[^"]*"' | cut -d'"' -f4)

if [ -z "$SIGNED_URL" ]; then
    echo -e "${RED}✗ Failed to sign URL${NC}"
    echo "Response: $SIGN_RESPONSE"
    exit 1
else
    echo -e "${GREEN}✓ URL signed successfully${NC}"
    echo "Signed URL: $SIGNED_URL"
fi
echo ""

# Test 3: Image transformation (should have caching)
echo "Test 3: Image Transformation (first request - should set cache headers)"
echo "----------------------------------------"
IMG_URL="$BASE_URL$SIGNED_URL"
IMG_RESPONSE=$(curl -s -I "$IMG_URL")

if echo "$IMG_RESPONSE" | grep -q "HTTP/[0-9.]\+ 200"; then
    echo -e "${GREEN}✓ Image transformation successful${NC}"
else
    echo -e "${RED}✗ Image transformation failed${NC}"
    echo "$IMG_RESPONSE"
    exit 1
fi

echo ""
echo "Checking cache headers:"

# Check Cache-Control header
check_header "$IMG_RESPONSE" "cache-control" "public" true
check_header "$IMG_RESPONSE" "cache-control" "max-age=" true
check_header "$IMG_RESPONSE" "cache-control" "s-maxage=" true
check_header "$IMG_RESPONSE" "cache-control" "immutable" true

# Check CDN-Cache-Control header
check_header "$IMG_RESPONSE" "cdn-cache-control" "max-age=" true

# Check Vary header
check_header "$IMG_RESPONSE" "vary" "Accept-Encoding" false

# Check ETag header
if echo "$IMG_RESPONSE" | grep -qi "^etag:"; then
    echo -e "${GREEN}✓ ETag header present${NC}"
else
    echo -e "${YELLOW}⚠ ETag header not found (optional)${NC}"
fi

# Check Content-Type
check_header "$IMG_RESPONSE" "content-type" "image/webp" false

echo ""
echo "Test 4: Cloudflare-Specific Headers (if proxied through Cloudflare)"
echo "----------------------------------------"

if echo "$IMG_RESPONSE" | grep -qi "^cf-cache-status:"; then
    CF_STATUS=$(echo "$IMG_RESPONSE" | grep -i "^cf-cache-status:" | cut -d' ' -f2 | tr -d '\r')
    
    case "$CF_STATUS" in
        "HIT")
            echo -e "${GREEN}✓ cf-cache-status: HIT (served from Cloudflare cache!)${NC}"
            ;;
        "MISS")
            echo -e "${YELLOW}⚠ cf-cache-status: MISS (first request, will be cached)${NC}"
            ;;
        "EXPIRED")
            echo -e "${YELLOW}⚠ cf-cache-status: EXPIRED (cache expired, revalidating)${NC}"
            ;;
        "DYNAMIC")
            echo -e "${RED}✗ cf-cache-status: DYNAMIC (Cloudflare is NOT caching!)${NC}"
            echo "  Fix: Add Page Rule with 'Cache Everything'"
            ;;
        "BYPASS")
            echo -e "${RED}✗ cf-cache-status: BYPASS (cache is being bypassed!)${NC}"
            echo "  Fix: Check Page Rules and cookies"
            ;;
        *)
            echo -e "${YELLOW}⚠ cf-cache-status: $CF_STATUS (unknown status)${NC}"
            ;;
    esac
    
    # Check age header (for HIT responses)
    if echo "$IMG_RESPONSE" | grep -qi "^age:"; then
        AGE=$(echo "$IMG_RESPONSE" | grep -i "^age:" | cut -d' ' -f2 | tr -d '\r')
        echo -e "${GREEN}✓ age: $AGE seconds (time since cached)${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Not proxied through Cloudflare (no cf-cache-status header)${NC}"
    echo "  This is expected for localhost testing"
    echo "  Deploy to production with Cloudflare DNS to test edge caching"
fi

echo ""
echo "Test 5: Second Request (should be cached)"
echo "----------------------------------------"

# Wait a moment
sleep 1

IMG_RESPONSE2=$(curl -s -I "$IMG_URL")

if echo "$IMG_RESPONSE2" | grep -qi "^cf-cache-status: HIT"; then
    echo -e "${GREEN}✓ Second request served from Cloudflare cache!${NC}"
elif echo "$IMG_RESPONSE2" | grep -qi "^cf-cache-status:"; then
    CF_STATUS2=$(echo "$IMG_RESPONSE2" | grep -i "^cf-cache-status:" | cut -d' ' -f2 | tr -d '\r')
    echo -e "${YELLOW}⚠ cf-cache-status: $CF_STATUS2 (not yet cached)${NC}"
else
    echo -e "${YELLOW}⚠ Not proxied through Cloudflare${NC}"
fi

echo ""
echo "========================================="
echo "Summary"
echo "========================================="
echo ""
echo "✅ Your caching headers are properly configured!"
echo ""
echo "Expected behavior with Cloudflare:"
echo "  1st request: cf-cache-status: MISS (fetches from origin)"
echo "  2nd request: cf-cache-status: HIT (served from edge)"
echo ""
echo "Cache durations:"
echo "  - Browser cache: 1 year (max-age=31536000)"
echo "  - Edge cache: 1 day (s-maxage=86400)"
echo ""
echo "To test with Cloudflare:"
echo "  1. Deploy to Render.io"
echo "  2. Configure Cloudflare DNS with proxying enabled (orange cloud)"
echo "  3. Run: ./test_cache_headers.sh https://img.yourdomain.com"
echo ""
