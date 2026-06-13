#!/usr/bin/env bash
# Update Alist cloud drive tokens
# Usage: ./scripts/update-alist-tokens.sh
set -euo pipefail

ALIST_URL="https://filter-manage-api.xyls.us.kg"
ALIST_USER="admin"
ALIST_PASS="alist123456"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

login() {
  local resp
  resp=$(curl -s -X POST "${ALIST_URL}/api/auth/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"${ALIST_USER}\",\"password\":\"${ALIST_PASS}\"}")

  TOKEN=$(echo "$resp" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
  if [ -z "$TOKEN" ]; then
    echo -e "${RED}AList login failed${NC}"
    echo "$resp"
    exit 1
  fi
}

check_storage() {
  local path="$1"
  local resp
  resp=$(curl -s -X POST "${ALIST_URL}/api/fs/list" \
    -H "Authorization: ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"path\":\"${path}\",\"page\":1,\"per_page\":1,\"refresh\":true}")

  local code
  code=$(echo "$resp" | grep -o '"code":[0-9]*' | head -1 | cut -d: -f2)
  if [ "$code" = "200" ]; then
    return 0
  else
    return 1
  fi
}

get_storage_id() {
  local mount_path="$1"
  curl -s "${ALIST_URL}/api/admin/storage/list" \
    -H "Authorization: ${TOKEN}" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for s in data.get('data',{}).get('content',[]):
    if s['mount_path'] == '$mount_path':
        print(s['id'])
        break
" 2>/dev/null
}

get_storage_config() {
  local id="$1"
  curl -s "${ALIST_URL}/api/admin/storage/get?id=${id}" \
    -H "Authorization: ${TOKEN}"
}

update_storage_cookie() {
  local id="$1"
  local driver="$2"
  local mount_path="$3"
  local new_cookie="$4"

  local addition
  addition=$(python3 -c "
import json, sys
print(json.dumps(json.dumps({'cookie': '''${new_cookie}''', 'root_folder_id': '', 'authorization': '', 'dpop': '', 'debug': False})))
")

  local resp
  resp=$(curl -s -X POST "${ALIST_URL}/api/admin/storage/update" \
    -H "Authorization: ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{
      \"id\": ${id},
      \"mount_path\": \"${mount_path}\",
      \"order\": 0,
      \"driver\": \"${driver}\",
      \"cache_expiration\": 30,
      \"status\": \"work\",
      \"addition\": ${addition},
      \"remark\": \"\",
      \"disabled\": false,
      \"disable_index\": false,
      \"enable_sign\": false,
      \"order_by\": \"\",
      \"order_direction\": \"\",
      \"extract_folder\": \"\",
      \"web_proxy\": false,
      \"webdav_policy\": \"302_redirect\",
      \"proxy_range\": false,
      \"down_proxy_url\": \"\",
      \"down_proxy_sign\": true
    }")

  local code
  code=$(echo "$resp" | grep -o '"code":[0-9]*' | head -1 | cut -d: -f2)
  if [ "$code" = "200" ]; then
    return 0
  else
    echo -e "${RED}Update failed: ${resp}${NC}"
    return 1
  fi
}

check_and_update_doubao() {
  echo -e "\n${YELLOW}=== Checking Doubao (豆包) ===${NC}"

  if check_storage "/doubao"; then
    echo -e "${GREEN}Doubao token is valid, no update needed${NC}"
    return
  fi

  echo -e "${RED}Doubao token has expired!${NC}"
  echo ""
  echo "Please update the token:"
  echo "  1. Open https://www.doubao.com in browser"
  echo "  2. Press F12 -> Application -> Cookies -> www.doubao.com"
  echo "  3. Find 'LARK_SUITE_ACCESS_TOKEN' and 'LARK_SUITE_DPOP'"
  echo "  4. Copy the FULL cookie string (or just these two values)"
  echo ""
  echo "  Tip: In DevTools Console, run:"
  echo "  copy(document.cookie)"
  echo "  Then paste below."
  echo ""
  read -rp "Paste new cookie (or press Enter to skip): " NEW_COOKIE

  if [ -z "$NEW_COOKIE" ]; then
    echo -e "${YELLOW}Skipped${NC}"
    return
  fi

  local id
  id=$(get_storage_id "/doubao")
  if [ -z "$id" ]; then
    echo -e "${RED}Doubao storage not found in AList${NC}"
    return
  fi

  if update_storage_cookie "$id" "DoubaoNew" "/doubao" "$NEW_COOKIE"; then
    echo -e "${GREEN}Doubao token updated successfully${NC}"
    if check_storage "/doubao"; then
      echo -e "${GREEN}Verified: Doubao is now working${NC}"
    else
      echo -e "${RED}Warning: Update applied but verification failed${NC}"
    fi
  fi
}

check_and_update_quark() {
  echo -e "\n${YELLOW}=== Checking Quark (夸克) ===${NC}"

  if check_storage "/quark"; then
    echo -e "${GREEN}Quark token is valid${NC}"
    return
  fi

  echo -e "${RED}Quark token has expired!${NC}"
  echo "Please update via AList admin panel: ${ALIST_URL}/@manage/storages"
}

check_and_update_uc() {
  echo -e "\n${YELLOW}=== Checking UC ===${NC}"

  if check_storage "/uc"; then
    echo -e "${GREEN}UC token is valid${NC}"
    return
  fi

  echo -e "${RED}UC token has expired!${NC}"
  echo "Please update via AList admin panel: ${ALIST_URL}/@manage/storages"
}

check_and_update_wukong() {
  echo -e "\n${YELLOW}=== Checking WuKong (悟空) ===${NC}"

  if check_storage "/wukong"; then
    echo -e "${GREEN}WuKong token is valid${NC}"
    return
  fi

  echo -e "${RED}WuKong token has expired!${NC}"
  echo "Please update via AList admin panel: ${ALIST_URL}/@manage/storages"
}

# --- Main ---
echo -e "${YELLOW}AList Token Health Check${NC}"
echo "Server: ${ALIST_URL}"
echo ""

login
echo -e "${GREEN}Login OK${NC}"

check_and_update_doubao
check_and_update_quark
check_and_update_uc
check_and_update_wukong

echo -e "\n${YELLOW}=== Summary ===${NC}"
echo "Storages using OAuth/refresh tokens (usually don't expire):"
echo "  aliyundrive, 115, baidu, lanzou, yandex"
echo ""
echo "Storages using cookies (need periodic refresh):"
echo "  doubao (~7 days), quark, uc, wukong"
