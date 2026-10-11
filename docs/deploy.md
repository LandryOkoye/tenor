# Deploying Tenor (hackathon demo)

Goal: a public **demo** that costs nothing on chain — the API serves mock Panta data with seeded
baskets (no real money, no live key). Switching to real Panta later is only env vars.

```
Railway project
├── Postgres            (Railway plugin → DATABASE_URL)
├── tenor-api           (backend/Dockerfile, start: api)        ← public URL
├── tenor-worker        (backend/Dockerfile, start: sh -c "seed && exec worker")
└── frontend            (handled by the frontend agent: Vercel/Netlify, VITE_API_BASE_URL = tenor-api URL)
```

## Steps (Railway, ~10 minutes)

1. Push `main` to GitHub (`git push origin main`).
2. railway.com → **New Project → Deploy from GitHub repo** → `LandryOkoye/tenor`.
3. **Add Postgres**: New → Database → PostgreSQL.
4. Configure the first service as **tenor-api**:
   - Settings → Root Directory: `backend` (Railway picks up `backend/Dockerfile`).
   - Settings → Networking → **Generate Domain** (this is the API URL).
   - Variables: the "shared" block below.
5. **New → GitHub repo** again (same repo) for **tenor-worker**:
   - Root Directory `backend`; Settings → Deploy → **Custom Start Command**:
     `sh -c "seed && exec worker"` (seeds the demo baskets once, then runs the jobs).
   - Variables: same shared block.
6. Check `https://<api-domain>/readyz` → `{"db":true,...}` and `/v1/baskets` lists 3 baskets.
7. Give the frontend agent the API URL; set `CORS_ORIGINS` on both services to the frontend URL.

### Shared variables (demo, mock Panta)

```
APP_ENV=staging                      # production refuses mock data on purpose
DATABASE_URL=${{Postgres.DATABASE_URL}}
PANTA_MOCK=true
JWT_SECRET=<random 32+ chars>
FEE_WALLET_PUBKEY=<any wallet pubkey you control>
ADMIN_WALLETS=<your wallet pubkey>
CORS_ORIGINS=https://<frontend-domain>
RUST_LOG=info
```

`PORT` is provided by Railway; the API binds `0.0.0.0:$PORT` automatically.

### Switching to real Panta later (real money)

```
APP_ENV=production
PANTA_MOCK=false
PANTA_API_KEY=pk_live_...
SOLANA_RPC_URL=https://<quicknode endpoint>
SOLANA_WS_URL=wss://<quicknode endpoint>
PANTA_PROGRAM_IDS=6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp
VAULT_PROGRAM_ID=AGrL8gCofvFTvsLDXmwThAWd1Wdz9ULPYwHWCzVDnF7u   # only once deployed + audited
CRANK_KEYPAIR=/secrets/crank.json                              # worker only
```

Change the worker start command back to `worker` (the seed is mock-only).

## Other hosts

The same image runs anywhere: `docker build -t tenor-backend backend/`, then run it twice
(`api` and `worker`) with the variables above and a Postgres URL. Render: two "Web/Background
Worker" services from `backend/Dockerfile`. Fly: `fly launch` in `backend/`, plus a second
process group `worker`.
