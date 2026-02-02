# K2 Application

## Cài đặt và Chạy

### Yêu cầu
*   Rust & Cargo
*   Node.js (v18+) & Pnpm/Npm
*   Tauri CLI (`npm install -g @tauri-apps/cli`)
*   C++ Build Tools (Windows) hoặc Xcode (macOS)

### Chạy Development

1.  Cài đặt dependencies:
    ```bash
    cd k2-app-tauri
    npm install
    ```

2.  Chạy ứng dụng (Desktop):
    ```bash
    npm run tauri dev
    ```

3.  Chạy ứng dụng (Android):
    ```bash
    npm run tauri android dev
    ```

### Build Production

```bash
npm run tauri build
```
File cài đặt sẽ được tạo tại `k2-app-tauri/src-tauri/target/release/bundle/`.
