# Khởi tạo K2Node

Quá trình khởi tạo `K2Node` là bước thiết lập toàn bộ stack giao thức P2P để sẵn sàng cho các hoạt động mạng. Quá trình này diễn ra chủ yếu trong hàm `with_data_dir`.

## Quy trình khởi tạo chi tiết

Dưới đây là các bước kỹ thuật diễn ra khi một Node được khởi động:

1. **Định danh (Identity)**: Sử dụng `IdentityManager` để tải hoặc sinh `SecretKey`. Khóa này được lưu trữ bền vững trong OS Secure Store (qua `amulet`) và file backup mã hóa, giúp duy trì Node ID qua các lần restart.
2. **Khám phá Peer (Discovery)**: Thiết lập `DhtDiscovery` sử dụng Pkarr DHT và DNS Relay. Điều này cho phép node tìm thấy các peer khác trên internet mà không cần máy chủ trung tâm.
3. **Mở cổng mạng (Endpoint)**: Khởi tạo iroh `Endpoint`, bind vào một cổng UDP để xử lý kết nối QUIC.
4. **Đăng ký Giao thức (ALPN)**: Đăng ký các mã định danh giao thức:
   - `iroh_blobs`: Chuyển file.
   - `iroh_gossip`: Chat/Sự kiện.
   - `iroh_docs`: Đồng bộ DB (Xem thêm: [Tích hợp iroh-docs](iroh-docs-integration.md)).
5. **Dịch vụ phụ trợ (Sub-services)**:
   - **Gossip**: Spawn bộ xử lý tin nhắn gossip.
   - **Blobs**: Thiết lập bộ nhớ lưu trữ file (mặc định là MemStore).
   - **Docs**: Khởi tạo hệ thống đồng bộ dữ liệu (hỗ trợ lưu RAM hoặc lưu file tùy cấu hình).
6. **Điều phối (Router)**: Thiết lập `Router` để tự động điều hướng dữ liệu đến đúng module dựa trên giao thức ALPN.

## Cấu hình lưu trữ (Persistence)

Hệ thống K2 hiện đại sử dụng cơ chế lưu trữ **Strictly Persistent** (Luôn lưu xuống đĩa), đảm bảo không mất dữ liệu khi tắt ứng dụng.

### 1. Đường dẫn lưu trữ tự động (Standardized Path)
Node sử dụng thư viện `directories` để tìm đường dẫn `AppData` phù hợp với từng hệ điều hành:
- **Windows**: `%APPDATA%\k2\network\data` (Thư mục Roaming chuẩn).
- **macOS/Linux**: Theo tiêu chuẩn XDG hoặc Application Support của hệ thống.

### 2. Cơ chế Linh hoạt cho Testing và Guest Mode (`K2_DATA_DIR`)
Để hỗ trợ việc chạy nhiều Node trên cùng một máy (test P2P) hoặc chế độ khách (Guest Mode), hệ thống ưu tiên kiểm tra biến môi trường `K2_DATA_DIR`.

**Khi `K2_DATA_DIR` được thiết lập:**
1. **Isolated Identity**: Hệ thống tự động chuyển sang chế độ **Danh tính cô lập**. Nó sẽ **bỏ qua OS Secure Store (Amulet)** để tránh ghi đè hoặc làm ô nhiễm danh tính chính của người dùng.
2. **Local Identity**: Một `SecretKey` mới sẽ được sinh ra và chỉ lưu duy nhất trong file `identity.enc` bên trong thư mục `K2_DATA_DIR`.
3. **Independent Storage**: Toàn bộ database (iroh-docs) và tệp tin (iroh-blobs) được lưu trữ riêng biệt, tránh lỗi Lock Database.

**Ví dụ chạy Node thứ 2 (Guest) trên Windows (PowerShell):**
```powershell
$env:K2_DATA_DIR="C:\\K2_Guest_1"; .\\k2-app.exe
```
*(Lưu ý: Trong PowerShell, dùng dấu `;` để ngăn cách lệnh thay vì `&&` của CMD).*

**Giải pháp dài hạn:**
Triển khai **Guest Mode** (Chế độ khách): Tự động phát hiện lock và chuyển sang sử dụng bộ nhớ tạm (In-memory) hoặc thư mục temp cho app thứ hai.

---
**Sources:** Antigravity AI Analysis 2026
**Raw:** [2026-04-24-k2-node-initialization-analysis.md](../../raw/k2/2026-04-24-k2-node-initialization-analysis.md); [2026-04-24-amulet-identity-implementation.md](../../raw/k2/2026-04-24-amulet-identity-implementation.md)
**Updated:** 2026-04-25
