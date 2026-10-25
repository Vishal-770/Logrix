# Database Web UI Guide: Connecting to AWS Aurora PostgreSQL

This guide explains how to fetch the PostgreSQL connection string directly from AWS, establish a secure local tunnel into the private VPC, and run a lightweight browser-based database UI container (`sosedoff/pgweb`) to inspect indexed tables and execute SQL queries.

No passwords or secrets are hardcoded in this guide. Credentials are retrieved dynamically using the AWS CLI and AWS Secrets Manager.

---

## Architecture Overview

AWS Aurora PostgreSQL is deployed inside **private VPC subnets** without public IP addresses for enterprise security. To inspect the database through a graphical web interface on your workstation:

```
[ Your Browser (http://localhost:8081) ]
                  │
                  ▼
[ Local Docker Container: sosedoff/pgweb ]
                  │
                  ▼ (localhost:5432)
[ kubectl port-forward tunnel ]
                  │
                  ▼ (Private VPC via EKS)
[ AWS Aurora Serverless v2 PostgreSQL ]
```

---

## Step 1: Fetch the Connection String from AWS

Run this command in your terminal. It dynamically queries **AWS Secrets Manager** and **AWS RDS** to construct the complete connection string:

```bash
aws secretsmanager get-secret-value \
  --secret-id $(aws secretsmanager list-secrets --region us-east-1 --query "SecretList[?contains(Name, 'logrix-production-rds')].ARN | [0]" --output text) \
  --region us-east-1 \
  --query "SecretString" --output text | jq -r '"postgres://" + .username + ":" + (.password | @uri) + "@'$(aws rds describe-db-clusters --region us-east-1 --query "DBClusters[0].Endpoint" --output text)':5432/logrix"'
```

This outputs your full connection string in the following format:
```text
postgres://<username>:<encoded-password>@<aurora-cluster-endpoint>:5432/logrix
```

---

## Step 2: Open the Database Tunnel (Terminal 1)

Since Aurora does not accept direct public internet connections, run an ephemeral proxy pod inside the Kubernetes cluster and forward port `5432` to your local machine:

```bash
# 1. Fetch the cluster endpoint dynamically
DB_HOST=$(aws rds describe-db-clusters --region us-east-1 --query "DBClusters[0].Endpoint" --output text)

# 2. Start a TCP forwarder pod inside the cluster
kubectl run db-tunnel --image=alpine/socat --restart=Never -n logrix -- \
  -d -d TCP-LISTEN:5432,fork,reuseaddr TCP:${DB_HOST}:5432

# 3. Forward port 5432 to your local workstation
kubectl port-forward pod/db-tunnel 5432:5432 -n logrix
```

Leave this terminal tab open while you use the web interface.

---

## Step 3: Run the Web UI Container (Terminal 2)

Open a **new terminal tab** and run the `sosedoff/pgweb` container. Replace `<username>` and `<encoded-password>` with the values extracted in Step 1:

```bash
docker run --rm -it -p 8081:8081 --name pgweb \
  sosedoff/pgweb \
  --url "postgres://<username>:<encoded-password>@host.docker.internal:5432/logrix?sslmode=disable"
```

*Note for Linux users: If `host.docker.internal` is not mapped on your system, add `--add-host=host.docker.internal:host-gateway` to the `docker run` command, or connect to `127.0.0.1:5432`.*

---

## Step 4: Open in Your Browser

Open your browser and navigate to:
```text
http://localhost:8081
```

### Features Available in pgweb:
* **Schema & Table Browser**: Browse all tables on the left navigation bar (`checkpoints`, `supply_actions`, `user`, `reserve`, etc.).
* **Data Viewer**: Inspect rows, filter columns, and sort by block numbers or timestamps.
* **SQL Query Console**: Execute custom SQL queries with syntax highlighting and auto-completion.
* **Data Export**: Export query results directly to CSV or JSON.

---

## Step 5: Clean Up

When you are done exploring:
1. Stop the Docker container: Press `Ctrl + C` in Terminal 2.
2. Stop the port-forwarding tunnel: Press `Ctrl + C` in Terminal 1.
3. Delete the temporary tunnel pod from Kubernetes:
```bash
kubectl delete pod db-tunnel -n logrix
```
