# Security Policy

## Supported Versions

Security updates and critical patches are actively provided for the following releases:

| Version | Supported          |
| ------- | ------------------ |
| 0.6.x   | Yes                |
| < 0.6.0 | No                 |

## Reporting a Vulnerability

The Logrix team takes security seriously. If you discover a security vulnerability within Logrix, please report it privately:

1. **Do not disclose the issue publicly** on GitHub issues, pull requests, discussions, or social channels.
2. Send an email to **security@logrix.io** with details of the vulnerability, including:
   - A description of the issue and potential impact
   - Reproduction steps, proof-of-concept code, or configuration
   - Affected components, versions, or environments
3. We will acknowledge receipt within 48 hours and provide an estimated timeline for evaluation and fix.
4. Once a fix is verified, a patched release will be issued alongside public attribution and disclosure.

## Security Best Practices for Operators

- **Secret Management**: Never commit RPC provider keys, database credentials, or webhook secrets in plaintext manifests. Always use Kubernetes Secrets, HashiCorp Vault, or AWS Secrets Manager.
- **Network Isolation**: Restrict direct inbound access to internal database (`postgres:5432`) and message broker (`rabbitmq:5672`) pods. Expose only the GraphQL API service (`:4000`) via ingress or API gateway.
- **Webhook Signatures**: Always verify `X-Logrix-Signature` headers with your shared secret to prevent forged webhook replay attacks.
