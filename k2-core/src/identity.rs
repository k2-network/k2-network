use anyhow::{Context, Result};
use iroh::SecretKey;
use std::path::PathBuf;
use std::fs;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};

#[cfg(target_os = "windows")]
use amulet::{AmuletStore, WindowsStore};

/// IdentityManager handles loading and saving the node's SecretKey.
/// It uses OS-level secure storage (via Amulet) as primary storage
/// and an encrypted file as a backup.
pub struct IdentityManager;

impl IdentityManager {
    const SERVICE_NAME: &'static str = "com.k2.network";
    const KEY_NAME: &'static str = "node_identity";
    
    // Fixed encryption key for backup file (In production this should be more dynamic)
    const BACKUP_ENC_KEY: &'static [u8; 32] = b"k2-network-identity-storage-key!";
    const STATIC_NONCE: &'static [u8; 12] = b"k2-id-nonce!";

    /// Get the roaming directory for K2
    pub fn get_roaming_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join("com.k2.network");
            }
        }
        
        // Fallback for other OS or if env var missing
        PathBuf::from("com.k2.network")
    }

    /// Load the secret key from storage, or generate a new one if not found.
    pub fn load_or_generate() -> Result<SecretKey> {
        // 1. Try to load from Amulet (OS Secure Store)
        if let Ok(Some(key)) = Self::load_from_amulet() {
            println!("[Identity] 🔐 Loaded identity from OS Secure Store");
            return Ok(key);
        }

        // 2. Try to load from Backup File (Encrypted)
        if let Ok(key) = Self::load_from_backup_file() {
            println!("[Identity] 💾 Recovered identity from encrypted backup file");
            // Sync back to Amulet for future use
            let _ = Self::save_to_amulet(&key);
            return Ok(key);
        }

        // 3. Generate new if both failed
        println!("[Identity] ✨ Generating new identity (first time initialization)");
        let new_key = SecretKey::generate(&mut rand::rng());
        
        // Save to both locations
        Self::save_to_amulet(&new_key).context("Failed to save identity to OS store")?;
        Self::save_to_backup_file(&new_key).context("Failed to save identity to backup file")?;
        
        Ok(new_key)
    }

    /// Load from OS Store via Amulet
    fn load_from_amulet() -> Result<Option<SecretKey>> {
        #[cfg(target_os = "windows")]
        {
            let mut store = WindowsStore::new();
            match store.get_password(Self::SERVICE_NAME, Self::KEY_NAME) {
                Ok(Some(secret_hex)) => {
                    let bytes = hex::decode(secret_hex.as_str())
                        .map_err(|e| anyhow::anyhow!("Invalid hex in Amulet: {}", e))?;
                    let key_bytes: [u8; 32] = bytes.try_into()
                        .map_err(|_| anyhow::anyhow!("Invalid key length in Amulet"))?;
                    let key = SecretKey::from_bytes(&key_bytes);
                    Ok(Some(key))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    println!("[Identity] ⚠️ Amulet error: {:?}", e);
                    Ok(None)
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(None)
        }
    }

    /// Save to OS Store via Amulet
    fn save_to_amulet(key: &SecretKey) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let mut store = WindowsStore::new();
            let key_hex = hex::encode(key.to_bytes());
            store.set_password(Self::SERVICE_NAME, Self::KEY_NAME, &key_hex)
                .map_err(|e| anyhow::anyhow!("Amulet save failed: {:?}", e))?;
        }
        Ok(())
    }

    /// Save to encrypted backup file
    fn save_to_backup_file(key: &SecretKey) -> Result<()> {
        let dir = Self::get_roaming_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        
        let path = dir.join("identity.enc");
        let data = key.to_bytes(); // 32 bytes
        
        // Encrypt
        let cipher_key = Key::<Aes256Gcm>::from_slice(Self::BACKUP_ENC_KEY);
        let cipher = Aes256Gcm::new(cipher_key);
        let nonce = Nonce::from_slice(Self::STATIC_NONCE);
        
        let ciphertext = cipher.encrypt(nonce, data.as_ref())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
        fs::write(path, ciphertext)?;
        Ok(())
    }

    /// Load from encrypted backup file
    fn load_from_backup_file() -> Result<SecretKey> {
        let path = Self::get_roaming_dir().join("identity.enc");
        if !path.exists() {
            return Err(anyhow::anyhow!("Backup file not found"));
        }
        
        let ciphertext = fs::read(path)?;
        
        // Decrypt
        let cipher_key = Key::<Aes256Gcm>::from_slice(Self::BACKUP_ENC_KEY);
        let cipher = Aes256Gcm::new(cipher_key);
        let nonce = Nonce::from_slice(Self::STATIC_NONCE);
        
        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
            
        let key_bytes: [u8; 32] = plaintext.try_into()
            .map_err(|_| anyhow::anyhow!("Invalid key data in backup"))?;
            
        Ok(SecretKey::from_bytes(&key_bytes))
    }
}
