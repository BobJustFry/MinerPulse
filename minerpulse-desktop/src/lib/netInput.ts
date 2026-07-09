const IPV4_RE = /^(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;

/** Keep only digits and dots while typing an IPv4 address. */
export function formatIpv4Input(raw: string): string {
  let out = "";
  let octets = 0;
  for (const ch of raw) {
    if (ch >= "0" && ch <= "9") {
      out += ch;
    } else if (ch === "." && out.length > 0 && out[out.length - 1] !== "." && octets < 3) {
      out += ".";
      octets += 1;
    }
  }
  const parts = out.split(".");
  return parts
    .slice(0, 4)
    .map((p) => p.slice(0, 3))
    .join(".");
}

export function isValidIpv4(value: string): boolean {
  return IPV4_RE.test(value.trim());
}

export function isValidNetmask(value: string): boolean {
  const ip = value.trim();
  if (!IPV4_RE.test(ip)) return false;
  const n = ipv4ToUint32(ip);
  if (n === 0) return false;
  const inv = (~n >>> 0) + 1;
  return (inv & (inv - 1)) === 0;
}

function ipv4ToUint32(ip: string): number {
  return ip.split(".").reduce((acc, oct) => (acc << 8) + Number(oct), 0) >>> 0;
}

export type NetFieldKey = "net_ip" | "net_mask" | "net_gate" | "net_dns";

export function netFieldValid(key: NetFieldKey, value: string): boolean {
  const v = value.trim();
  if (!v) return false;
  if (key === "net_mask") return isValidNetmask(v);
  return isValidIpv4(v);
}
