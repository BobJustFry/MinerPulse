export function betaSelfServiceEnabled(): boolean {
  const raw = process.env.BETA_SELF_SERVICE?.trim().toLowerCase();
  return raw === "true" || raw === "1" || raw === "yes";
}

export function betaMaxDevices(): number {
  const parsed = Number(process.env.BETA_MAX_DEVICES ?? 10);
  if (!Number.isFinite(parsed) || parsed < 1) return 10;
  return Math.min(Math.trunc(parsed), 50);
}

export function betaConfig() {
  return {
    selfService: betaSelfServiceEnabled(),
    maxDevices: betaMaxDevices(),
  };
}
