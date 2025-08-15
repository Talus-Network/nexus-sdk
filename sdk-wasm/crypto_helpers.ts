// Enhanced crypto helpers for WASM integration
// Provides CLI-compatible functionality for browser environment

class NexusCryptoHelpers {
  private masterKey: string | null;
  private sessions: Map<string, any>;

  constructor() {
    this.masterKey = null;
    this.sessions = new Map();
  }

  // Generate random master key using Web Crypto API
  async generateRandomMasterKey() {
    const keyBytes = new Uint8Array(32);
    crypto.getRandomValues(keyBytes);
    const hex = Array.from(keyBytes)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    return hex;
  }

  // Securely store master key in localStorage with encryption
  async storeMasterKeySecurely(masterKeyHex: string) {
    try {
      // Use Web Crypto API to encrypt the master key
      const keyData = new TextEncoder().encode(masterKeyHex);
      const iv = crypto.getRandomValues(new Uint8Array(12));

      // Generate a storage key from browser-specific data
      const storageKeyMaterial = await crypto.subtle.importKey(
        "raw",
        new TextEncoder().encode(navigator.userAgent + location.origin),
        { name: "PBKDF2" },
        false,
        ["deriveKey"]
      );

      const storageKey = await crypto.subtle.deriveKey(
        {
          name: "PBKDF2",
          salt: new TextEncoder().encode("nexus-wasm-salt"),
          iterations: 100000,
          hash: "SHA-256",
        },
        storageKeyMaterial,
        { name: "AES-GCM", length: 256 },
        false,
        ["encrypt", "decrypt"]
      );

      const encrypted = await crypto.subtle.encrypt(
        { name: "AES-GCM", iv: iv },
        storageKey,
        keyData
      );

      // Store IV + encrypted data
      const combined = new Uint8Array(iv.length + encrypted.byteLength);
      combined.set(iv, 0);
      combined.set(new Uint8Array(encrypted), iv.length);

      localStorage.setItem(
        "nexus-master-key",
        btoa(String.fromCharCode(...combined))
      );
      return { success: true, message: "Master key stored securely" };
    } catch (error) {
      console.error("Failed to store master key:", error);
      return { success: false, error: error.message };
    }
  }

  // Load master key from secure localStorage
  async loadMasterKeySecurely() {
    try {
      console.log("🔍 Starting loadMasterKeySecurely...");

      const storedData = localStorage.getItem("nexus-master-key");
      if (!storedData) {
        console.log("❌ No master key found in localStorage");
        return { success: false, error: "No master key found" };
      }

      console.log("📦 Found stored data, length:", storedData.length);

      // Validate stored data format
      if (typeof storedData !== "string" || storedData.length === 0) {
        console.error("Invalid stored data format");
        return { success: false, error: "Invalid stored data format" };
      }

      let combined;
      try {
        console.log("🔓 Decoding stored data...");
        const decoded = atob(storedData);
        console.log("✅ Decoded data length:", decoded.length);

        combined = new Uint8Array(
          decoded.split("").map((c) => c.charCodeAt(0))
        );
        console.log("✅ Combined array length:", combined.length);
      } catch (decodeError) {
        console.error("❌ Failed to decode stored data:", decodeError);
        return { success: false, error: "Failed to decode stored data" };
      }

      // Validate combined data length
      if (combined.length < 12) {
        console.error(
          "❌ Stored data too short for IV, length:",
          combined.length
        );
        return { success: false, error: "Stored data too short" };
      }

      const iv = combined.slice(0, 12);
      const encrypted = combined.slice(12);

      console.log(
        "🔑 IV length:",
        iv.length,
        "Encrypted length:",
        encrypted.length
      );

      if (encrypted.length === 0) {
        console.error("❌ No encrypted data found");
        return { success: false, error: "No encrypted data found" };
      }

      // Check if Web Crypto API is available
      if (!crypto || !crypto.subtle) {
        console.error("❌ Web Crypto API not available");
        return { success: false, error: "Web Crypto API not available" };
      }

      console.log("🔐 Web Crypto API available, recreating storage key...");

      // Recreate storage key with better error handling
      let storageKeyMaterial;
      try {
        const keyMaterial = new TextEncoder().encode(
          navigator.userAgent + location.origin
        );

        storageKeyMaterial = await crypto.subtle.importKey(
          "raw",
          keyMaterial,
          { name: "PBKDF2" },
          false,
          ["deriveKey"]
        );
        console.log("✅ Storage key material imported successfully");
      } catch (importError) {
        console.error("❌ Failed to import key material:", importError);
        return { success: false, error: "Failed to import key material" };
      }

      let storageKey;
      try {
        console.log("🔐 Deriving storage key...");
        storageKey = await crypto.subtle.deriveKey(
          {
            name: "PBKDF2",
            salt: new TextEncoder().encode("nexus-wasm-salt"),
            iterations: 100000,
            hash: "SHA-256",
          },
          storageKeyMaterial,
          { name: "AES-GCM", length: 256 },
          false,
          ["encrypt", "decrypt"]
        );
        console.log("✅ Storage key derived successfully");
      } catch (deriveError) {
        console.error("❌ Failed to derive storage key:", deriveError);
        return { success: false, error: "Failed to derive storage key" };
      }

      let decrypted;
      try {
        console.log("🔓 Attempting to decrypt master key...");
        console.log(
          "🔑 IV:",
          Array.from(iv)
            .map((b: any) => b.toString(16).padStart(2, "0"))
            .join("")
        );
        console.log("🔒 Encrypted data length:", encrypted.length);

        decrypted = await crypto.subtle.decrypt(
          { name: "AES-GCM", iv: iv },
          storageKey,
          encrypted
        );
        console.log(
          "✅ Decryption successful, decrypted length:",
          decrypted.byteLength
        );
      } catch (decryptError) {
        console.error("❌ Failed to decrypt master key:", decryptError);
        console.error("❌ Error name:", decryptError.name);
        console.error("❌ Error message:", decryptError.message);

        // Provide more specific error information
        if (decryptError.name === "OperationError") {
          console.error(
            "❌ This is an OperationError - likely browser context changed or data corrupted"
          );
          return {
            success: false,
            error:
              "Decryption failed - this usually means the browser context has changed or the stored key is corrupted. Please clear storage and regenerate the master key.",
          };
        }

        return {
          success: false,
          error: `Decryption failed: ${decryptError.message}`,
        };
      }

      const masterKeyHex = new TextDecoder().decode(decrypted);

      // Validate the decrypted master key
      if (!masterKeyHex || masterKeyHex.length === 0) {
        console.error("❌ Decrypted master key is empty");
        return { success: false, error: "Decrypted master key is empty" };
      }

      console.log(
        "✅ Master key loaded successfully, length:",
        masterKeyHex.length
      );
      return { success: true, masterKey: masterKeyHex };
    } catch (error) {
      console.error("❌ Failed to load master key:", error);
      return { success: false, error: error.message };
    }
  }

  // CLI-compatible crypto init function
  async cryptoInitKey(wasmModule: any, force = false) {
    try {
      console.log("🔐 Starting CLI-compatible crypto init-key...");
      console.log("🔍 Force flag:", force);

      // CLI-parity: Check for existing keys first
      const existingKeys = await this.checkExistingKeys();
      console.log("🔍 Existing keys check:", existingKeys);

      if (existingKeys.hasAnyKey && !force) {
        console.log(
          "❌ Key already exists and force=false, aborting (CLI-parity)"
        );
        return {
          success: false,
          error: "KeyAlreadyExists",
          message:
            "A different persistent key already exists; re-run with --force if you really want to replace it",
          requires_force: true,
        };
      }

      // Call WASM key_init to check status and get instructions
      const initResult = wasmModule.key_init(force);
      const parsedResult = JSON.parse(initResult);

      if (!parsedResult.success) {
        console.log("❌ Key init failed:", parsedResult.message);
        return parsedResult;
      }

      // If we got a master key to store, store it securely
      if (parsedResult.action === "store_key" && parsedResult.master_key) {
        console.log("🔍 CLI-parity: Storing new master key...");

        // CLI-parity: Clean up any existing passphrase entries first
        await this.cleanupPassphraseEntries();

        const storeResult = await this.storeMasterKeySecurely(
          parsedResult.master_key
        );
        if (!storeResult.success) {
          return { success: false, error: storeResult.error };
        }

        console.log(
          "✅ Master key generated and stored successfully (CLI-parity)"
        );
        return {
          success: true,
          message: "32-byte master key saved to secure storage",
          master_key_preview: parsedResult.master_key.substring(0, 16) + "...",
          cli_compatible: true,
        };
      }

      return parsedResult;
    } catch (error) {
      console.error("❌ Crypto init-key failed:", error);
      return { success: false, error: error.message };
    }
  }

  // CLI-parity: Check for existing keys (like CLI's keyring check)
  async checkExistingKeys() {
    try {
      const masterKeyExists = localStorage.getItem("nexus-master-key") !== null;
      const passphraseExists =
        localStorage.getItem("nexus-passphrase") !== null;

      console.log("🔍 CLI-parity: Checking existing keys...");
      console.log("  - Master key exists:", masterKeyExists);
      console.log("  - Passphrase exists:", passphraseExists);

      return {
        hasAnyKey: masterKeyExists || passphraseExists,
        masterKeyExists,
        passphraseExists,
      };
    } catch (error) {
      console.error("❌ Failed to check existing keys:", error);
      return {
        hasAnyKey: false,
        masterKeyExists: false,
        passphraseExists: false,
      };
    }
  }

  // CLI-parity: Clean up passphrase entries (like CLI's cleanup)
  async cleanupPassphraseEntries() {
    try {
      console.log("🔍 CLI-parity: Cleaning up passphrase entries...");

      // Remove any stale passphrase entries (CLI-parity)
      const passphraseKeys = [
        "nexus-passphrase",
        "nexus-cli-store-passphrase",
        "nexus-cli-passphrase",
      ];

      let cleanedCount = 0;
      for (const key of passphraseKeys) {
        if (localStorage.getItem(key) !== null) {
          localStorage.removeItem(key);
          console.log("  - Cleaned up:", key);
          cleanedCount++;
        }
      }

      if (cleanedCount > 0) {
        console.log(
          "✅ CLI-parity: Cleaned up {} passphrase entries",
          cleanedCount
        );
      } else {
        console.log("✅ CLI-parity: No passphrase entries to clean up");
      }

      return { success: true, cleanedCount };
    } catch (error) {
      console.error("❌ Failed to cleanup passphrase entries:", error);
      return { success: false, error: error.message };
    }
  }

  // CLI-compatible key status check
  async cryptoKeyStatus(wasmModule: any) {
    try {
      const wasmStatus = wasmModule.key_status();
      const parsedStatus = JSON.parse(wasmStatus);

      // Add JavaScript-specific information
      const jsStatus = await this.getStatus();

      return {
        ...parsedStatus,
        js_storage: {
          master_key_exists: jsStatus.masterKeyExists,
          sessions_exist: jsStatus.sessionsExist,
          crypto_api_available: jsStatus.cryptoApiAvailable,
        },
        compatible_with_cli: true,
      };
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  // Status check (internal)
  async getStatus() {
    const masterKeyExists = localStorage.getItem("nexus-master-key") !== null;
    const sessionsExist = localStorage.getItem("nexus-sessions") !== null;

    return {
      masterKeyExists,
      sessionsExist,
      cryptoApiAvailable: !!(crypto && crypto.subtle),
      userAgent: navigator.userAgent,
      origin: location.origin,
    };
  }
}

// Export for use
(window as any).NexusCryptoHelpers = NexusCryptoHelpers;
