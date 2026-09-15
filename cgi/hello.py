import os
import sys

body = sys.stdin.buffer.read().decode("utf-8", errors="replace")
path_info = os.environ.get("PATH_INFO", "/")
method = os.environ.get("REQUEST_METHOD", "GET")

print("Content-Type: text/html")
print()
print("<h1>Hello from Python CGI!</h1>")
print(f"<p>Method: {method}</p>")
print(f"<p>PATH_INFO: {path_info}</p>")
print(f"<p>Body: {body}</p>")
