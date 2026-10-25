# Database Web UI Guide: Connecting to AWS Aurora PostgreSQL

This guide provides the simplest, 2-step workflow to view and query the private AWS Aurora PostgreSQL database in your browser using `pgweb`.

No passwords or secrets are hardcoded. Credentials are automatically injected from the cluster secret (`logrix-secrets`).

---

## Step 1: Start pgweb and Forward to Your Browser

Run these two commands in separate terminal tabs (or together in the background):

### Terminal 1: Launch the pgweb UI Pod in Kubernetes
```bash
kubectl run pgweb-ui --image=sosedoff/pgweb -n logrix \
  --env="PGWEB_DATABASE_URL=$(kubectl get secret logrix-secrets -n logrix -o jsonpath='{.data.DATABASE_URL}' | base64 -d)" \
  --command -- pgweb --bind=0.0.0.0 --listen=8081
```

### Terminal 2: Forward Port to Browser
```bash
kubectl port-forward pod/pgweb-ui 8081:8081 -n logrix
```

Now open your browser to:
```text
http://localhost:8081
```

You will see:
* All database tables on the left (`checkpoints`, `supply_actions`, `user`, etc.).
* Interactive SQL query console to run queries and export data.

---

## Step 2: Clean Up When Done

When you are finished viewing the database:
1. In Terminal 2 (port-forward): Press `Ctrl + C`.
2. Delete the pod from the cluster:
```bash
kubectl delete pod pgweb-ui -n logrix
```

---

## Useful SQL Queries to Run in pgweb

Open the **Query** tab in `pgweb` and run any of these queries:

### Check Latest Indexed Checkpoint
```sql
SELECT chain_id, last_indexed_block, last_indexed_hash, updated_at 
FROM logrix_checkpoints;
```

### Check Latest Indexed Blockchain Events (SupplyAction)
```sql
SELECT entity_id, block_number, data->>'amount' AS amount, data->>'user' AS user_address 
FROM logrix_entities 
WHERE entity_type = 'SupplyAction' 
ORDER BY block_number DESC 
LIMIT 10;
```

### Check Protocol Overview Totals
```sql
SELECT entity_id, data 
FROM logrix_entities 
WHERE entity_type = 'Protocol';
```
