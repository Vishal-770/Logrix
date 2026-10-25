# Database Web UI Guide: Connecting to AWS Aurora PostgreSQL

This guide documents the fastest, most reliable ways to connect a graphical browser UI to the private AWS Aurora PostgreSQL database in the Logrix cluster.

No passwords or secrets are hardcoded in this guide. Credentials are dynamically injected from Kubernetes secrets and AWS Secrets Manager.

---

## Architecture Overview

AWS Aurora PostgreSQL is deployed inside **private VPC subnets** without public IP addresses for enterprise security. To inspect the database through a graphical web interface:

```
[ Your Browser (http://localhost:8081) ]
                  │
                  ▼
[ kubectl port-forward (localhost:8081) ]
                  │
                  ▼ (Encrypted Kube-API Tunnel)
[ Ephemeral Pod inside EKS: pgweb-ui ]
                  │
                  ▼ (Internal Private VPC: Port 5432)
[ AWS Aurora Serverless v2 PostgreSQL ]
```

---

## Method 1: The Direct In-Cluster Pod (Recommended: Fastest & Zero Local Setup)

This is the cleanest and fastest method. You **do not need Docker installed on your workstation**, you never have to copy-paste passwords, and no local network bridging is required.

### Step 1: Launch the `pgweb` Pod in Kubernetes (Terminal 1)
Run this command to start an ephemeral `pgweb` container directly inside the AWS EKS cluster. It automatically reads the connection string from the existing `logrix-secrets` secret:

```bash
kubectl run pgweb-ui --rm -i --restart=Never --image=sosedoff/pgweb -n logrix \
  --env="PGWEB_DATABASE_URL=$(kubectl get secret logrix-secrets -n logrix -o jsonpath='{.data.DATABASE_URL}' | base64 -d)" \
  --command -- pgweb --bind=0.0.0.0 --listen=8081
```

Once connected, you will see output like:
```text
Connecting to server...
Connected to PostgreSQL 16.8
Checking database objects...
Starting server...
To view database open http://0.0.0.0:8081/ in browser
```
*Leave this terminal open while using the web interface.*

### Step 2: Forward the Port to Your Browser (Terminal 2)
Open a second terminal tab and forward port `8081` to your workstation:

```bash
kubectl port-forward pod/pgweb-ui 8081:8081 -n logrix
```

### Step 3: Open in Browser
Open your browser and navigate to:
```text
http://localhost:8081
```

### Step 4: Clean Up
When you are finished viewing the database:
1. In Terminal 2 (port-forward): Press `Ctrl + C`.
2. In Terminal 1 (`pgweb-ui`): Press `Ctrl + C`.
3. Because `--rm` was specified, Kubernetes **automatically deletes the pod**. No resources or costs are left behind.

---

## Method 2: Running `pgweb` via Local Docker Container

If you prefer running the `pgweb` container on your local machine using Docker:

### Step 1: Fetch the Connection String from AWS
Extract your cluster endpoint, username, and URL-encoded password dynamically:

```bash
aws secretsmanager get-secret-value \
  --secret-id $(aws secretsmanager list-secrets --region us-east-1 --query "SecretList[?contains(Name, 'logrix-production-rds')].ARN | [0]" --output text) \
  --region us-east-1 \
  --query "SecretString" --output text | jq -r '"postgres://" + .username + ":" + (.password | @uri) + "@'$(aws rds describe-db-clusters --region us-east-1 --query "DBClusters[0].Endpoint" --output text)':5432/logrix"'
```

This outputs:
```text
postgres://<username>:<encoded-password>@<aurora-cluster-endpoint>:5432/logrix
```

### Step 2: Open the Local Database Tunnel (Terminal 1)
Forward port `5432` from the private VPC to your workstation:

```bash
DB_HOST=$(aws rds describe-db-clusters --region us-east-1 --query "DBClusters[0].Endpoint" --output text)

kubectl run db-tunnel --image=alpine/socat --restart=Never -n logrix -- \
  -d -d TCP-LISTEN:5432,fork,reuseaddr TCP:${DB_HOST}:5432

kubectl port-forward pod/db-tunnel 5432:5432 -n logrix
```

### Step 3: Run the Local Docker Container (Terminal 2)
```bash
docker run --rm -it -p 8081:8081 --name pgweb \
  sosedoff/pgweb \
  --url "postgres://<username>:<encoded-password>@host.docker.internal:5432/logrix?sslmode=disable"
```

Open `http://localhost:8081` in your browser.

### Step 4: Clean Up
1. Stop Docker: Press `Ctrl + C` in Terminal 2.
2. Stop Tunnel: Press `Ctrl + C` in Terminal 1.
3. Delete the bridge pod:
```bash
kubectl delete pod db-tunnel -n logrix
```

---

## 3. Useful SQL Queries to Run in pgweb

Once connected to `pgweb`, open the **Query** tab and run these queries to inspect indexed blockchain data:

### Inspect Latest Block Checkpoint
```sql
SELECT chain_id, last_indexed_block, last_indexed_hash, updated_at 
FROM logrix_checkpoints;
```

### Inspect Computed GraphQL Entities (SupplyAction, Protocol, User)
```sql
SELECT entity_type, entity_id, block_number, data 
FROM logrix_entities 
ORDER BY block_number DESC 
LIMIT 10;
```

### Inspect Filtered Deposits
```sql
SELECT entity_id, block_number, data->>'amount' AS amount, data->>'user' AS user_address 
FROM logrix_entities 
WHERE entity_type = 'SupplyAction' 
ORDER BY block_number DESC 
LIMIT 10;
```

### Check Protocol Summary Totals
```sql
SELECT entity_id, data 
FROM logrix_entities 
WHERE entity_type = 'Protocol';
```
