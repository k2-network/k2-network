# Node ID trong K2

Node ID là mã định danh duy nhất cho mỗi nút (node) trong mạng lưới P2P của K2. Nó được sử dụng để xác thực, kết nối và định tuyến tin nhắn giữa các peer.

## Cơ chế sinh Node ID

Node ID trong K2 thực chất là **Public Key** của Iroh node. Quá trình sinh diễn ra như sau:

1. **Khởi tạo Secret Key**: Khi một `K2Node` được tạo mới trong `k2-core`, hệ thống sẽ sinh một cặp khóa Ed25519 bằng hàm:
   ```rust
   let secret_key = SecretKey::generate(&mut rand::rng());
   ```
2. **Trích xuất Public Key**: Node ID là chuỗi string đại diện cho Public Key tương ứng với Secret Key đó.
3. **Lưu trữ bảo mật**: Node ID hiện được quản lý bởi `IdentityManager` với cơ chế lưu trữ hai lớp:
   - **Lớp chính**: Lưu trong OS Secure Store (Windows Credential Manager) thông qua thư viện `amulet`.
   - **Lớp dự phòng**: Lưu file mã hóa AES-256-GCM tại `%APPDATA%/com.k2.network/identity.enc`.
   - **Tính bền vững**: Node ID giờ đây cố định và được duy trì qua các lần khởi động lại ứng dụng.

## Luồng chi tiết trong k2-app

Luồng hoạt động từ frontend đến backend:

1. **Frontend (React)**: 
   - Khi ứng dụng khởi chạy, `App.tsx` gọi lệnh `invoke("init_node")`.
2. **Backend (Tauri/Rust)**:
   - Lệnh `init_node` gọi `K2Node::new().await`.
   - `k2-core` sinh `SecretKey` và khởi tạo Iroh `Endpoint`.
   - `init_node` lấy Node ID bằng cách gọi `node.my_id()`.
   - Một phiên bản rút gọn (10 ký tự đầu) được trả về cho frontend để hiển thị trên Header.
3. **Truy vấn lại**:
   - Khi cần ID đầy đủ (ví dụ để chia sẻ), frontend gọi `get_my_node_id`.
   - Backend truy xuất node từ `AppState` và gọi `node.my_id()` để trả về chuỗi đầy đủ.

---
**Sources:** K2 Team 2026; Internal Code Analysis
**Raw:** [2026-04-23-node-id-generation-analysis.md](../../raw/k2/2026-04-23-node-id-generation-analysis.md); [2026-04-24-amulet-identity-implementation.md](../../raw/k2/2026-04-24-amulet-identity-implementation.md)
**Updated:** 2026-04-24
