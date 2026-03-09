# Vercel + Fly.io + Supabase Deployment

This deployment splits the app into:

- `frontend/` on Vercel
- `backend/` on Fly.io
- PostgreSQL on Supabase

The frontend calls the backend over HTTPS using `NEXT_PUBLIC_API_URL`.

## 1. Supabase database

1. Create a new Supabase project.
2. Open the project dashboard and copy the Postgres connection string from `Connect`.
3. Use the direct connection string first. Supabase recommends direct connections for persistent services such as VMs and Fly.io Machines. If you run into IPv4-only networking constraints, switch to the Supavisor session pooler connection string instead.
4. Keep the final value ready for Fly secrets as `DATABASE_URL`.

Example format:

```env
DATABASE_URL=postgresql://postgres:<password>@db.<project-ref>.supabase.co:5432/postgres
```

## 2. Fly.io backend

This repository now includes:

- `fly.toml`
- `backend/Dockerfile`
- `/healthz` for health checks
- `PORT` fallback support in the backend config

Before the first deploy:

1. Install `flyctl`.
2. Authenticate with `fly auth login`.
3. From the repository root, create the app:

```bash
fly launch --no-deploy
```

Use these values when prompted:

- App name: your preferred unique name, such as `mrr-fantasy-api`
- Region: pick the closest region to your users or your Supabase region
- PostgreSQL: `No`
- Redis: `No`
- Dockerfile: keep `backend/Dockerfile`

Then set secrets:

```bash
fly secrets set \
  DATABASE_URL="postgresql://postgres:<password>@db.<project-ref>.supabase.co:5432/postgres" \
  JWT_SECRET="<long-random-secret>"
```

Deploy:

```bash
fly deploy
```

After deploy, get the backend URL:

```bash
fly status
```

Your API will be available at a Fly hostname such as:

```text
https://mrr-fantasy-api.fly.dev
```

Quick verification:

```bash
curl https://mrr-fantasy-api.fly.dev/healthz
curl https://mrr-fantasy-api.fly.dev/api/players
```

## 3. Vercel frontend

Create a new Vercel project from this repository.

Important settings:

- Framework: `Next.js`
- Root Directory: `frontend`
- Node version: `20` or newer

Set this environment variable in Vercel:

```env
NEXT_PUBLIC_API_URL=https://mrr-fantasy-api.fly.dev
```

Then deploy.

If you use the Vercel CLI with a monorepo, Vercel's current monorepo docs recommend linking the repository and setting the project root directory to the app subdirectory you want to deploy.

## 4. Custom domains

After both services are working:

1. Add your frontend domain in Vercel, for example `app.example.com`.
2. Add your backend domain in Fly.io, for example `api.example.com`.
3. Point DNS records to the providers as instructed by Vercel and Fly.
4. Update `NEXT_PUBLIC_API_URL` in Vercel to your final backend domain:

```env
NEXT_PUBLIC_API_URL=https://api.example.com
```

Redeploy the frontend after changing the variable.

## 5. Ongoing deploys

- Frontend updates: push to the connected Git branch and Vercel redeploys `frontend/`
- Backend updates: run `fly deploy` from the repo root

## Notes

- The backend runs SQL migrations automatically at startup, so your Supabase schema is created during the first successful boot.
- The current backend CORS configuration allows all origins. That works for Vercel, but if you want to lock it down later, make the allowed origin configurable and set it to your frontend domain.
