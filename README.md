# MRR Fantasy

A full-stack multiplayer fantasy football application where users build 6-player squads and compete in leagues.

## Architecture

- **Frontend**: Next.js 16 (App Router) + TypeScript + Tailwind CSS + Framer Motion
- **Backend**: Rust (Axum) + SQLx + PostgreSQL
- **Auth**: JWT-based authentication with Argon2 password hashing

## Points System

| Action               | Points |
| -------------------- | ------ |
| Goal (Forward)       | +10    |
| Goal (Midfielder)    | +8     |
| Goal (Defender/GK)   | +12    |
| Assist               | +5     |
| Clean Sheet (DEF/GK) | +2     |
| Save (GK)            | +2     |
| Tackle Won           | +2     |

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18+)
- [PostgreSQL](https://www.postgresql.org/) (v14+)

## Setup

### 1. Database

```bash
# Create the database
createdb mrr_fantasy
```

### 2. Backend

```bash
cd backend

# Copy and edit environment variables
cp .env.example .env
# Edit .env with your DATABASE_URL

# Run the server (auto-runs migrations and seeds data)
cargo run
```

The backend starts on `http://localhost:8080`.

### 3. Frontend

```bash
cd frontend

# Install dependencies
npm install

# Run dev server
npm run dev
```

The frontend starts on `http://localhost:3000`.

## Raspberry Pi Deployment

Production-ready Raspberry Pi deployment files are included in:

- `deploy_pi.sh` (one-shot installer/deployer)
- `deploy/raspberry-pi/README.md`
- `deploy/raspberry-pi/mrrfantasy-backend.service`
- `deploy/raspberry-pi/mrrfantasy-frontend.service`
- `deploy/raspberry-pi/mrrfantasy.nginx.conf`

## API Endpoints

Auth is a JWT bearer token from `/api/auth/login`. "admin" routes additionally
require the account's `is_admin` flag.

### Auth — `/api/auth`
- `POST /register` — create account
- `POST /login` — returns a JWT
- `POST /reset-password` — change a password; needs the current password, or an admin token

### Players — `/api/players`
- `GET /` — list players (query: `?position=FWD&search=haaland`)
- `GET /leaderboard` — players ranked by points
- `GET /:id` — player details

### Points — `/api/points`
- `GET /week/:week` — a gameweek's player points
- `GET /player/:id` — one player's history

### Teams — `/api/teams`
- `GET /lock-status` — whether squad changes are open (public)
- `POST /` — create a fantasy team
- `GET /my` — your team
- `PUT /:id/players` — set the squad: 6 starters + 3 bench
- `GET /:id/points` — team points
- `GET /:id/transfer` — free transfers left and what a further one costs
- `GET /:id/chips` — chip status
- `POST /:id/chips` — play a chip on the active gameweek
- `DELETE /:id/chips/:chip_type` — take a chip back, before its gameweek starts

### Leagues — `/api/leagues`
- `GET /:id` — league details (public)
- `GET /:id/leaderboard` — rankings (public)
- `GET /:id/gameweek/:week` — one gameweek across the league (public)
- `POST /` — create a league
- `POST /join` — join with an invite code
- `GET /my` — your leagues
- `GET /:league_id/members/:user_id/lineup` — a member's squad
- `GET /:league_id/members/:user_id/gameweek/:week` — a member's gameweek
- `GET /:league_id/gameweek/:week/scoreboard` — every member's gameweek side by side

### Accounting — `/api/accounting`
Futsal session dues, separate from the fantasy game (migration `020`).
- `GET /my-dues` — what you owe
- `POST /sessions` *(admin)* — record a session
- `GET /sessions` *(admin)* — list sessions
- `GET /sessions/:id` *(admin)* — session detail
- `DELETE /sessions/:id` *(admin)* — delete a session
- `POST /sessions/:id/players` *(admin)* — add a player to a session
- `DELETE /sessions/:session_id/players/:player_id` *(admin)* — remove one
- `PUT /sessions/:session_id/players/:player_id/pay` *(admin)* — mark paid/unpaid
- `GET /users` *(admin)* — known payers
- `GET /user-summary` *(admin)* — dues per user

### Admin — `/api/admin`
- `POST /gameweek` — create or activate a gameweek. A scored week keeps the dates it was scored under
- `GET /gameweeks` — list gameweeks
- `PUT /gameweek/:week/toggle` — open or close a gameweek
- `GET /gameweek/:week/stats` — stats entered for a week
- `POST /gameweek/:week/stats` — submit stats: scores the week, moves prices and budgets, opens the next
- `GET /lineup-lock` — current lock override and effective lock
- `PUT /lineup-lock` — force lineups open or closed

### Health
- `GET /healthz`

## Project Structure

```
MrrFantasy/
├── frontend/           # Next.js app
│   └── src/
│       ├── app/        # Pages (landing, login, register, dashboard, team, league, leaderboard)
│       ├── components/ # Reusable components (nav, player-card, formation, points-badge)
│       └── lib/        # API client & auth helpers
├── backend/            # Rust Axum server
│   └── src/
│       ├── auth/       # JWT, middleware, handlers
│       ├── models/     # Data models
│       ├── handlers/   # Route handlers
│       └── services/   # Business logic, scoring & seeding
├── migrations/         # PostgreSQL migrations, applied at boot
└── ops/                # One-off repair scripts, run by hand — see ops/README.md
```

## Tests

The suite is mostly SQL, so most of it needs a database:

```bash
cd backend
DATABASE_URL=postgres://localhost/mrr_fantasy cargo test
```

Without `DATABASE_URL` those tests **fail** rather than skip — an early return
from a `#[tokio::test]` is a pass, and a suite that reports green having checked
nothing is worse than one that does not run. To skip them deliberately, set
`MRR_SKIP_DB_TESTS=1`; the run then says what it did not cover.

`cargo run -- --migrate-and-seed` migrates and seeds a database, then exits
without serving. CI uses it to build the database the tests run against.
