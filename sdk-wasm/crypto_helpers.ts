// Crypto helpers for Nexus WASM integration.
//
// Thin convenience layer over the WASM-exported functions for Sui private key
// and Ed25519 tool signing key management via localStorage.

interface WasmModule {
  set_sui_private_key(raw: string): string;
  get_sui_private_key_b64(): string | undefined;
  sui_key_status(): string;
  remove_sui_private_key(): string;
  tool_auth_keygen(force: boolean): string;
  tool_auth_import_key(raw: string, force: boolean): string;
  tool_key_status(): string;
  remove_tool_signing_key(): string;
  sign_http_request(
    method: string,
    path: string,
    query: string,
    body: Uint8Array,
    toolId: string,
    keyId: string,
    ttlMs: number
  ): string;
  crypto_clear_all(): string;
}

class NexusCryptoHelpers {
  private wasm: WasmModule;

  constructor(wasmModule: WasmModule) {
    this.wasm = wasmModule;
  }

  // ---- Sui private key ----

  setSuiPrivateKey(raw: string) {
    return JSON.parse(this.wasm.set_sui_private_key(raw));
  }

  getSuiPrivateKeyB64(): string | undefined {
    return this.wasm.get_sui_private_key_b64();
  }

  suiKeyStatus() {
    return JSON.parse(this.wasm.sui_key_status());
  }

  removeSuiPrivateKey() {
    return JSON.parse(this.wasm.remove_sui_private_key());
  }

  // ---- Tool signing key ----

  toolAuthKeygen(force = false) {
    return JSON.parse(this.wasm.tool_auth_keygen(force));
  }

  toolAuthImportKey(raw: string, force = false) {
    return JSON.parse(this.wasm.tool_auth_import_key(raw, force));
  }

  toolKeyStatus() {
    return JSON.parse(this.wasm.tool_key_status());
  }

  removeToolSigningKey() {
    return JSON.parse(this.wasm.remove_tool_signing_key());
  }

  // ---- Signed HTTP ----

  signHttpRequest(
    method: string,
    path: string,
    query: string,
    body: Uint8Array,
    toolId: string,
    keyId: string,
    ttlMs = 30_000
  ) {
    return JSON.parse(
      this.wasm.sign_http_request(method, path, query, body, toolId, keyId, ttlMs)
    );
  }

  // ---- Wipe ----

  clearAll() {
    return JSON.parse(this.wasm.crypto_clear_all());
  }

  // ---- Status ----

  getStatus() {
    const sui = this.suiKeyStatus();
    const tool = this.toolKeyStatus();
    return {
      suiKeyExists: sui.exists ?? false,
      toolKeyExists: tool.exists ?? false,
    };
  }
}

(window as any).NexusCryptoHelpers = NexusCryptoHelpers;
