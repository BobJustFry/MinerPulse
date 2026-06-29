import { prisma } from "./prisma.js";

export type DeviceInput = {
  hwid: string;
  os?: string;
  os_version?: string;
  app_version?: string;
};

export class DeviceLimitError extends Error {
  code = "device_limit" as const;
}

export function parseDeviceFields(body: Record<string, unknown>): DeviceInput | null {
  const hwid = String(body.hwid ?? body.device_fingerprint ?? "").trim();
  if (hwid.length < 8) return null;
  return {
    hwid,
    os: body.os ? String(body.os) : undefined,
    os_version: body.os_version ? String(body.os_version) : undefined,
    app_version: body.app_version ? String(body.app_version) : undefined,
  };
}

export async function upsertUserDevice(
  userId: string,
  device: DeviceInput,
  opts?: { maxDevices?: number },
) {
  const existing = await prisma.device.findUnique({
    where: { userId_hwid: { userId, hwid: device.hwid } },
  });

  if (!existing && opts?.maxDevices != null) {
    const count = await prisma.device.count({ where: { userId } });
    if (count >= opts.maxDevices) {
      throw new DeviceLimitError();
    }
  }

  return prisma.device.upsert({
    where: { userId_hwid: { userId, hwid: device.hwid } },
    update: {
      lastSeenAt: new Date(),
      os: device.os,
      osVersion: device.os_version,
      appVersion: device.app_version,
    },
    create: {
      userId,
      hwid: device.hwid,
      os: device.os,
      osVersion: device.os_version,
      appVersion: device.app_version,
    },
  });
}

export async function findUserDevice(userId: string, hwid: string) {
  return prisma.device.findFirst({ where: { userId, hwid } });
}
