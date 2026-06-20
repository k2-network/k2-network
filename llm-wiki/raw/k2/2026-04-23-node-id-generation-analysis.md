---
Source: Internal Code Analysis
Collected: 2026-04-23
Published: 2026-04-23
---

# Node ID Generation in K2

Analysis of how `node-id` is generated and used in the K2 system.

## K2 Core Logic

In `k2-core/src/lib.rs`, the `K2Node` struct manages the node's identity.

1. **Generation**:
   When a `K2Node` is initialized (via `K2Node::new()` or `K2Node::with_data_dir()`), it generates a new `iroh::SecretKey`:
   ```rust
   let secret_key = SecretKey::generate(&mut rand::rng());
   ```

2. **Identification**:
   The `node-id` is the public key derived from this secret key, converted to a string:
   ```rust
   pub fn my_id(&self) -> String {
       self.secret_key.public().to_string()
   }
   ```

## K2 App Tauri Integration

The Tauri application interacts with `k2-core` to manage the node identity.

1. **Initialization**:
    The frontend `App.tsx` triggers the node initialization:
    ```typescript
    // k2-app/src/App.tsx
    const shortId = await invoke<string>("init_node");
    ```

  2. **Backend Command**:
    The `init_node` command in `k2-app/src-tauri/src/lib.rs` handles the creation:
   ```rust
   #[tauri::command]
   async fn init_node(state: State<'_, AppState>, _app: tauri::AppHandle) -> Result<String, String> {
       // ... checks if already initialized ...
       let node = K2Node::new().await.map_err(|e| e.to_string())?;
       let node_id = node.my_id();
       // ... stores in AppState ...
       Ok(short_id) // Returns shortened ID for UI
   }
   ```

3. **Full ID Retrieval**:
   When the full ID is needed (e.g., for sharing), the `get_my_node_id` command is used:
   ```rust
   #[tauri::command]
   async fn get_my_node_id(state: State<'_, AppState>) -> Result<String, String> {
       let node = state.node.lock().unwrap();
       let n = node.as_ref().ok_or("Node not initialized")?;
       Ok(n.my_id())
   }
   ```
