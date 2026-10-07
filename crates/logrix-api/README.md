# logrix-api

Production GraphQL API server, real-time WebSocket subscriptions, and Webhook management service for Logrix.

## Overview
Built with Tokio and Axum, this crate serves the developer-facing querying and management APIs.

## Features
- **Dynamic GraphQL Engine**: Dynamically generates GraphQL types, filters, and resolvers from user-supplied `schema.graphql` or `schema.yaml` files.
- **WebSocket Subscriptions (`/ws`)**: Real-time event broadcasting over WebSockets using PostgreSQL `LISTEN/NOTIFY`.
- **Query Guardrails**: Enforces query depth limits (`max_depth: 7`) and complexity analysis to protect against denial-of-service queries.
- **Webhook Management API**: REST endpoints for registering, listing, and auditing webhook subscriptions.

## Endpoints
- `GET /` and `GET /graphiql` - Interactive GraphiQL IDE
- `POST /graphql` - GraphQL query execution
- `GET /ws` - WebSocket real-time subscription endpoint
- `POST /api/v1/webhooks` - Register a new webhook endpoint
- `GET /api/v1/webhooks` - List active webhook endpoints (with masked secrets)
- `DELETE /api/v1/webhooks/:id` - Delete a webhook endpoint
- `GET /api/v1/webhooks/deliveries` - Audit trail of recent webhook deliveries
- `GET /healthz` & `GET /readyz` - Kubernetes liveness and readiness probes
- `GET /metrics` - Prometheus metrics export
