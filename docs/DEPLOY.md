# Deploy K2 Network lên VPS (Ubuntu)

## Yêu cầu
- VPS Ubuntu 20.04+
- Tên miền đã trỏ A record về IP VPS (`103.163.118.194`)
- Quyền root

---

## Bước 1 — Cài Docker trên VPS

```bash
ssh root@103.163.118.194 -p 22

# Cài Docker
curl -fsSL https://get.docker.com | sh

# Cài Docker Compose plugin
apt-get install -y docker-compose-plugin

# Kiểm tra
docker --version
docker compose version
```

---

## Bước 2 — Upload code lên VPS

**Trên máy local**, chạy:

```bash
# Clone hoặc rsync code lên VPS
rsync -avz --exclude='target/' --exclude='node_modules/' --exclude='.git/' \
  /home/namle/k2-network/k2-network/ \
  root@103.163.118.194:/opt/k2-network/
```

---

## Bước 3 — Cấu hình trên VPS

```bash
ssh root@103.163.118.194

cd /opt/k2-network

# Tạo file .env từ example
cp .env.example .env

# Sửa .env — thay các giá trị CHANGE_THIS
nano .env

# Tạo JWT secrets ngẫu nhiên
echo "JWT_SECRET=$(openssl rand -hex 64)"
echo "JWT_REFRESH_SECRET=$(openssl rand -hex 64)"
# Copy 2 giá trị trên vào .env
```

---

## Bước 4 — Sửa nginx.conf — thay tên miền

```bash
# Thay k2team.xyz bằng tên miền thật
sed -i 's/k2team.xyz/tendomain.com/g' nginx.conf
```

---

## Bước 5 — Lấy SSL certificate (Let's Encrypt)

```bash
# Tạo thư mục certbot
mkdir -p certbot/www certbot/conf

# Chạy nginx tạm để verify domain (chưa có SSL)
# Tạm thời comment block HTTPS trong nginx.conf
docker compose up -d nginx

# Lấy cert
docker compose run --rm certbot certonly \
  --webroot \
  --webroot-path=/var/www/certbot \
  --email your@email.com \
  --agree-tos \
  --no-eff-email \
  -d tendomain.com \
  -d www.tendomain.com

# Bỏ comment block HTTPS trong nginx.conf
# Restart nginx
docker compose restart nginx
```

---

## Bước 6 — Chạy toàn bộ stack

```bash
cd /opt/k2-network

# Build và chạy
docker compose up -d --build

# Xem logs
docker compose logs -f

# Xem từng service
docker compose logs -f backend
docker compose logs -f postgres
```

---

## Kiểm tra

```bash
# Kiểm tra services đang chạy
docker compose ps

# Test API
curl https://tendomain.com/api/health

# Xem log backend
docker compose logs backend --tail=50
```

---

## Update code mới

```bash
# Trên máy local — upload code mới
rsync -avz --exclude='target/' --exclude='node_modules/' --exclude='.git/' \
  /home/namle/k2-network/k2-network/ \
  root@103.163.118.194:/opt/k2-network/

# Trên VPS — rebuild và restart
ssh root@103.163.118.194
cd /opt/k2-network
docker compose up -d --build backend frontend
```

---

## Database

```bash
# Kết nối vào PostgreSQL
docker compose exec postgres psql -U k2user -d k2db

# Xem tables
\dt

# Xem users
SELECT id, username, email, node_id, created_at FROM users;
```

---

## Cấu trúc files

```
/opt/k2-network/
├── docker-compose.yml       # Main compose file
├── Dockerfile.backend       # Build Rust backend
├── Dockerfile.frontend      # Build React frontend
├── nginx.conf               # Nginx reverse proxy (HTTPS)
├── nginx.frontend.conf      # Nginx config trong frontend container
├── .env                     # Secrets (KHÔNG commit lên git)
├── .env.example             # Template .env
├── certbot/                 # SSL certificates
│   ├── www/
│   └── conf/
├── scripts/
│   └── init.sql             # PostgreSQL schema
├── k2-web-server/           # Rust backend
└── k2-app-web/              # React frontend
```
