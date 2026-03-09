# Raspberry Pi Deployment (Production)

This app runs as:
- `mrrfantasy-backend` (Rust + Axum on `127.0.0.1:8080`)
- `mrrfantasy-frontend` (Next.js on `127.0.0.1:3000`)
- `nginx` as the public reverse proxy on port `80` (`443` optional)

## 1. Prepare Raspberry Pi OS

Use Raspberry Pi OS 64-bit (Bookworm recommended).

```bash
sudo apt update
sudo apt install -y git curl build-essential pkg-config libssl-dev libpq-dev \
  postgresql postgresql-contrib nginx
```

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
```

Install Node.js 20:

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
node -v
npm -v
```

## 2. Clone and build

```bash
cd /home/pi
git clone <YOUR_REPO_URL> MrrFantasy
cd MrrFantasy

# Backend build
cd backend
cargo build --release
cd ..

# Frontend build
cd frontend
npm ci
npm run build
cd ..
```

## 3. Configure PostgreSQL

```bash
sudo -u postgres psql -c "CREATE DATABASE mrr_fantasy;"
sudo -u postgres psql -c "CREATE USER mrrfantasy WITH PASSWORD 'CHANGE_ME_STRONG_PASSWORD';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE mrr_fantasy TO mrrfantasy;"
```

## 4. Create environment files

Backend env:

```bash
sudo cp deploy/raspberry-pi/backend.env.example /etc/mrrfantasy-backend.env
sudo nano /etc/mrrfantasy-backend.env
```

Frontend env:

```bash
sudo cp deploy/raspberry-pi/frontend.env.example /etc/mrrfantasy-frontend.env
sudo nano /etc/mrrfantasy-frontend.env
```

For nginx reverse proxy, keep:

```bash
NEXT_PUBLIC_API_URL=
```

(empty value means frontend calls `/api/...` on the same host)

## 5. Install systemd services

```bash
sudo cp deploy/raspberry-pi/mrrfantasy-backend.service /etc/systemd/system/
sudo cp deploy/raspberry-pi/mrrfantasy-frontend.service /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable mrrfantasy-backend mrrfantasy-frontend
sudo systemctl start mrrfantasy-backend mrrfantasy-frontend
```

Check status:

```bash
sudo systemctl status mrrfantasy-backend
sudo systemctl status mrrfantasy-frontend
```

## 6. Configure nginx

```bash
sudo cp deploy/raspberry-pi/mrrfantasy.nginx.conf /etc/nginx/sites-available/mrrfantasy
sudo ln -s /etc/nginx/sites-available/mrrfantasy /etc/nginx/sites-enabled/mrrfantasy
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl restart nginx
```

Edit `server_name` in `/etc/nginx/sites-available/mrrfantasy` to your domain or Pi IP.

## 7. Logs and operations

```bash
journalctl -u mrrfantasy-backend -f
journalctl -u mrrfantasy-frontend -f
```

Deploy updates:

```bash
cd /home/pi/MrrFantasy
git pull

cd backend && cargo build --release && cd ..
cd frontend && npm ci && npm run build && cd ..

sudo systemctl restart mrrfantasy-backend mrrfantasy-frontend
```

## 8. Optional HTTPS (Let's Encrypt)

If you have a domain:

```bash
sudo apt install -y certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com -d www.your-domain.com
```
