# Security

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use GitHub's private vulnerability reporting ("Report a vulnerability" under the repository's Security tab). You will get an answer within a week.

## How Nodex protects your code

- Nodex runs a server on `127.0.0.1` only. Every API call needs a random session token, and requests with a foreign `Host` or `Origin` are refused (DNS-rebinding defence).
- The desktop window gets the token directly; it is never put in a URL. Native features (folder picker, Save dialog, links) are granted to that local page only, and pages from anywhere else open in your browser.
- Nodex sends nothing about your code anywhere. It goes online only to check for updates and, when you click Install, to download a debug adapter, which is checked against a pinned SHA-256 hash.
- Updates are signed; the app refuses an update whose signature does not match the key built into it.
