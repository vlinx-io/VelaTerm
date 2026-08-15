//! Desktop-only command wrappers controlling the browser remote-access Web service.

import { invoke } from "./transport";

/** Web service status matching Rust `WebServerStatus` in camelCase. */
export interface WebServerStatus {
  running: boolean;
  port: number | null;
  /** Primary candidate access URL (first in urls). */
  url: string | null;
  /** Every candidate URL across network interfaces; choose one on the accessing device's subnet. VPN/tunnel interfaces come last. */
  urls: string[];
  /** Server self-signed certificate SHA-256 fingerprint (uppercase colon-separated hexadecimal) for verification; null when stopped. */
  fingerprint: string | null;
  /** Error message of the most recent failed auto-start (e.g. port in use); null after any successful start. */
  autostartError: string | null;
  /** Last persisted port from app settings, used to prefill the port field after a restart. */
  savedPort: number | null;
  /** Whether the persisted enabled flag will auto-start the service on the next launch. */
  autoStart: boolean;
  /** URL scheme of the running service ("https" for LAN TLS, "http" for the plaintext modes); null when
   * stopped. Explicit so URL synthesis for late-appearing interfaces never guesses from the URL list. */
  scheme: string | null;
}

/**
 * Start the password-protected Web service on the LAN and return its access URL.
 * `lanHttp=false`: LAN with self-signed TLS, the browser remote-access default.
 * `lanHttp=true`: plaintext LAN HTTP for the native mobile shell, whose WebView cannot bypass a self-signed
 * certificate; see architecture §20.
 */
export function webServerStart(
  password: string,
  port?: number,
  lanHttp = false,
): Promise<WebServerStatus> {
  return invoke<WebServerStatus>("web_server_start", {
    password,
    port: port ?? null,
    lanHttp,
  });
}

/** Stop the Web service. */
export function webServerStop(): Promise<void> {
  return invoke<void>("web_server_stop");
}

/** Query current status. */
export function webServerStatus(): Promise<WebServerStatus> {
  return invoke<WebServerStatus>("web_server_status");
}

/** Selectable network interface matching Rust `NetworkInterface` in camelCase. */
export interface NetworkInterface {
  /** OS-reported interface name (e.g. `en0`, `utun3`, `tailscale0`), shown for recognition. */
  name: string;
  /** IPv4 address; feeds `webPairingCreate`'s address argument. */
  ip: string;
  /** Point-to-point interface without a broadcast address, usually a VPN/tunnel; marked in the selector. */
  vpn: boolean;
}

/** List interface candidates for the IP selector, filtered identically to the status URL list. */
export function networkInterfacesList(): Promise<NetworkInterface[]> {
  return invoke<NetworkInterface[]>("network_interfaces_list");
}

/** Pairing result: browser URL containing token/server public key in the URL fragment, plus the device token. */
export interface PairingInfo {
  url: string;
  deviceToken: string;
}

/** Registered device matching Rust `DeviceEntry` in camelCase; client-reported identifiers are display-only. */
export interface DeviceEntry {
  deviceId: string;
  name: string;
  firstSeenAt: number;
  lastSeenAt: number;
}

/**
 * Generate a browser E2EE pairing link. `address` selects a network-interface IP (default: first LAN address);
 * `rotate=true` invalidates an unused old token. The service must already be running.
 */
export function webPairingCreate(
  address?: string,
  rotate = false,
): Promise<PairingInfo> {
  return invoke<PairingInfo>("web_pairing_create", {
    address: address ?? null,
    rotate,
  });
}

/** List paired devices that have actually connected. */
export function webDevicesList(): Promise<DeviceEntry[]> {
  return invoke<DeviceEntry[]>("web_devices_list");
}

/** Revoke a paired device. */
export function webDeviceRevoke(deviceId: string): Promise<boolean> {
  return invoke<boolean>("web_device_revoke", { deviceId });
}
