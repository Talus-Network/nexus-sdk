// Enhanced crypto helpers for WASM integration
// Provides CLI-compatible functionality for browser environment

class NexusCryptoHelpers {
  private masterKey: string | null;
  private sessions: Map<string, any>;

  constructor() {
    this.masterKey = null;
    this.sessions = new Map();
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
      return { success: false, error: error.message };
    }
  }

  // Load master key from secure localStorage
  async loadMasterKeySecurely() {
    try {
      const storedData = localStorage.getItem("nexus-master-key");
      if (!storedData) {
        return { success: false, error: "No master key found" };
      }

      // Validate stored data format
      if (typeof storedData !== "string" || storedData.length === 0) {
        return { success: false, error: "Invalid stored data format" };
      }

      let combined;
      try {
        const decoded = atob(storedData);
        combined = new Uint8Array(
          decoded.split("").map((c) => c.charCodeAt(0))
        );
      } catch (decodeError) {
        return { success: false, error: "Failed to decode stored data" };
      }

      // Validate combined data length
      if (combined.length < 12) {
        return { success: false, error: "Stored data too short" };
      }

      const iv = combined.slice(0, 12);
      const encrypted = combined.slice(12);

      if (encrypted.length === 0) {
        return { success: false, error: "No encrypted data found" };
      }

      // Check if Web Crypto API is available
      if (!crypto || !crypto.subtle) {
        return { success: false, error: "Web Crypto API not available" };
      }

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
      } catch (importError) {
        return { success: false, error: "Failed to import key material" };
      }

      let storageKey;
      try {
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
      } catch (deriveError) {
        return { success: false, error: "Failed to derive storage key" };
      }

      let decrypted;
      try {
        decrypted = await crypto.subtle.decrypt(
          { name: "AES-GCM", iv: iv },
          storageKey,
          encrypted
        );
      } catch (decryptError) {
        // Provide more specific error information
        if (decryptError.name === "OperationError") {
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
        return { success: false, error: "Decrypted master key is empty" };
      }

      return { success: true, masterKey: masterKeyHex };
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  // CLI-compatible crypto init function
  async cryptoInitKey(wasmModule: any, force = false) {
    try {
      // CLI-parity: Check for existing keys first
      const existingKeys = await this.checkExistingKeys();

      if (existingKeys.hasAnyKey && !force) {
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
        return parsedResult;
      }

      // If we got a master key to store, store it securely
      if (parsedResult.action === "store_key" && parsedResult.master_key) {
        const storeResult = await this.storeMasterKeySecurely(
          parsedResult.master_key
        );
        if (!storeResult.success) {
          return { success: false, error: storeResult.error };
        }

        return {
          success: true,
          message: "32-byte master key saved to secure storage",
          master_key_preview: parsedResult.master_key.substring(0, 16) + "...",
          cli_compatible: true,
        };
      }

      return parsedResult;
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  // CLI-parity: Check for existing keys (like CLI's keyring check)
  async checkExistingKeys() {
    try {
      const masterKeyExists = localStorage.getItem("nexus-master-key") !== null;
      const passphraseExists =
        localStorage.getItem("nexus-passphrase") !== null;

      return {
        hasAnyKey: masterKeyExists || passphraseExists,
        masterKeyExists,
        passphraseExists,
      };
    } catch (error) {
      return {
        hasAnyKey: false,
        masterKeyExists: false,
        passphraseExists: false,
      };
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
