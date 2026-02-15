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
| Clean Sheet (DEF/GK) | +6     |
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

## API Endpoints

### Auth
- `POST /api/auth/register` - Create account
- `POST /api/auth/login` - Login, returns JWT

### Players
- `GET /api/players` - List players (query: `?position=FWD&search=haaland`)
- `GET /api/players/:id` - Player details

### Teams (requires auth)
- `POST /api/teams` - Create fantasy team
- `GET /api/teams/my` - Get your team
- `PUT /api/teams/:id/players` - Set 6 players
- `GET /api/teams/:id/points` - Team points

### Leagues (requires auth for create/join)
- `POST /api/leagues` - Create league
- `POST /api/leagues/join` - Join with invite code
- `GET /api/leagues/:id` - League details
- `GET /api/leagues/:id/leaderboard` - Rankings

### Points
- `GET /api/points/week/:week` - Week points
- `GET /api/points/player/:id` - Player history

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
│       └── services/   # Business logic & seeding
└── migrations/         # PostgreSQL migrations
```
