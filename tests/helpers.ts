
import {
    createKeyPairSignerFromBytes,
    generateKeyPair,
    getAddressFromPublicKey,
    type KeyPairSigner,
  } from "@solana/kit";
  import fs from "fs";
  import path from "path";
  import os from "os";
  
  // The new library takes a brand-new approach to Solana key pairs and addresses,
  // which will feel quite different from the classes PublicKey and Keypair from version 1.x.
  // All key operations now use the native Ed25519 implementation in JavaScript’s
  // Web Crypto API.
  async function createKeypair() {
    const newKeypair: CryptoKeyPair = await generateKeyPair();
    const publicAddress = await getAddressFromPublicKey(newKeypair.publicKey);
  
    console.log(`Public key: ${publicAddress}`);
  }
  
  export async function loadKeypairFromFile(
    filePath: string,
  ): Promise<KeyPairSigner<string>> {
    // This is here so you can also load the default keypair from the file system.
    const resolvedPath = path.resolve(
      filePath.startsWith("~") ? filePath.replace("~", os.homedir()) : filePath,
    );
    const loadedKeyBytes = Uint8Array.from(
      JSON.parse(fs.readFileSync(resolvedPath, "utf8")),
    );
    // Here you can also set the second parameter to true in case you need to extract your private key.
    const keypairSigner = await createKeyPairSignerFromBytes(loadedKeyBytes);
    return keypairSigner;
  }
  
  export async function loadDefaultKeypair(): Promise<KeyPairSigner<string>> {
    return await loadKeypairFromFile("~/.config/solana/id.json");
  }
  
  