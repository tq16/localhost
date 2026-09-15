#!/bin/bash

BASE_URL="http://127.0.0.1:8080"

check_status() {
    name="$1"
    expected="$2"
    shift 2

    actual=$(curl -s -o /dev/null -w "%{http_code}" "$@")

    if [ "$actual" = "$expected" ]; then
        echo "PASS: $name ($actual)"
    else
        echo "FAIL: $name (expected $expected, got $actual)"
    fi
}

check_status "Home page" 200 "$BASE_URL/"
check_status "Static page" 200 "$BASE_URL/about.html"
check_status "Missing page" 404 "$BASE_URL/not-found.html"
check_status "Redirect" 302 "$BASE_URL/old-page"
check_status "Session" 200 "$BASE_URL/session"
check_status "Directory listing" 200 "$BASE_URL/uploads/"
check_status "CGI" 200 "$BASE_URL/cgi/hello.py/test"
check_status "Method not allowed" 405 -X POST "$BASE_URL/assets/"
check_status "Raw upload" 201 -X POST --data-binary "test upload" "$BASE_URL/uploads/test.txt"
check_status "Uploaded file" 200 "$BASE_URL/uploads/test.txt"
check_status "Delete file" 204 -X DELETE "$BASE_URL/uploads/test.txt"
check_status "Deleted file" 404 "$BASE_URL/uploads/test.txt"