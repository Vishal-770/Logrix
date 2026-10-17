# Security Policy

## Supported Versions

The previously documented security-maintenance policy lists the following line.
This checkout's Rust version is 0.7.3 and SDK version is 0.7.0; neither source
version establishes a published release or an extension of this support policy.
Confirm support for a deployed version with maintainers and current advisories.

| Version | Supported          |
| ------- | ------------------ |
| 0.6.x   | Yes                |
| 0.7.x   | Not declared in this policy |
| < 0.6.0 | No                 |

## Reporting a Vulnerability

If you discover a security vulnerability within Logrix, please report it privately:

1. **Do not disclose the issue publicly** on GitHub issues, pull requests, discussions, or social channels.
2. Open a private security advisory report via **[GitHub Security Advisories](https://github.com/Vishal-770/Logrix/security/advisories/new)** with:
   - A description of the issue and potential impact
   - Reproduction steps, proof-of-concept code, or configuration
   - Affected components, versions, or environments
3. We will acknowledge receipt within 48 hours and work on an evaluation and fix.
4. Once a fix is verified, a patched release will be issued alongside public attribution and release notes.

## Security Best Practices for Operators

- **Secret Management**: Never commit RPC provider keys, database credentials, or webhook secrets in plaintext manifests. Always use Kubernetes Secrets, HashiCorp Vault, or AWS Secrets Manager.
- **Network Isolation**: Restrict direct inbound access to internal database (`postgres:5432`) and message broker (`rabbitmq:5672`) pods. Expose only the GraphQL API service (`:4000`) via ingress or API gateway.
- **API Boundary**: The API and webhook management routes have no built-in authentication; GraphQL CORS is permissive. Restrict access and put authentication, authorization, TLS, rate limits and request-size controls at your network/application boundary. URL checks are not comprehensive SSRF protection; restrict outbound destinations too.
- **Webhook Signatures**: Verify `X-Logrix-Signature` with the shared secret and the exact request body. Enforce timestamp freshness and deduplicate deliveries; a valid HMAC by itself does not prevent replay. The crate's verifier checks a timestamp window, not a durable deduplication ledger. See [webhook verification](crates/logrix-webhook/README.md).
- **Sandbox and HA Limits**: WASM memory settings are checks, not allocation ceilings. Current Kubernetes election is not safe multi-ingester coordination. Review the [operational limits](README.md#operational-limits) and [deployment precautions](deploy/README.md) before exposure or scaling.
- **Secret Transport**: Secret env values override ConfigMaps, but Kubernetes Secret base64 is not encryption and Secrets Manager permissions do not inject values automatically. Keep real state, tfvars, keys and populated Secret files out of git and logs.
