import { Buffer } from "buffer";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/inter-tight";
import "@fontsource-variable/jetbrains-mono";
import "./index.css";
import { App } from "./App";

// @solana/web3.js expects Node's Buffer in the browser.
(globalThis as unknown as { Buffer: typeof Buffer }).Buffer ??= Buffer;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
