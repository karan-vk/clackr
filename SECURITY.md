# Security Policy

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, use GitHub's private vulnerability reporting for this repository:

1. Go to the [Security tab](https://github.com/karan-vk/clackr/security).
2. Click **"Report a vulnerability"** under **Advisories**.
3. Fill in as much detail as you can: affected version, platform, and
   reproduction steps.

This opens a private conversation with the maintainer(s) that isn't visible
to the public until a fix is ready, and lets us coordinate a disclosure
timeline and, if appropriate, request a CVE.

## Scope

clackr is a local, offline terminal application: it reads a config file and
word/quote lists from disk, renders a TUI, and writes back an updated config
file. It does not make network requests and does not execute untrusted
input as code. Realistic report categories include things like:

- Path traversal or unsafe file handling in config/asset loading
  (`config_io.rs`, the embedded/override language and theme loaders).
- Panics or undefined behavior reachable from malformed config files,
  theme files, or word/quote lists.
- Terminal-escape-sequence injection via untrusted word/quote content.

## Supported Versions

Only the latest published release is supported with security fixes. Given
the project is pre-1.0, please upgrade to the latest version before
reporting to confirm the issue still reproduces.
