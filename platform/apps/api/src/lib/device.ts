import { prisma } from "./prisma.js";
import { activeSubscription } from "./subscription.js";

export type DeviceInput = {
  hwid: string;
  os?: string;
  os_version?: string;
  app_version?: string;
  app_build?: number;
};

export class DeviceLimitError extends Error {
  code = "device_limit" as const;
}

export function effectiveMaxDevices(
  planMaxDevices: number,
  override: number | null | undefined,
): number {
  if (override != null && override > 0) {
    return override;
  }
  return planMaxDevices;
}

export async function maxDevicesForUser(userId: string): Promise<number> {
  const user = await prisma.user.findUnique({
    where: { id: userId },
    select: { maxDevicesOverride: true },
  });
  if (!user) return 1;
  const sub = await activeSubscription(userId);
  const planMax = sub?.plan.maxDevices ?? 1;
  return effectiveMaxDevices(planMax, user.maxDevicesOverride);
}

export function parseDeviceFields(body: Record<string, unknown>): DeviceInput | null {
  const hwid = String(body.hwid ?? body.device_fingerprint ?? "").trim();
  if (hwid.length < 8) return null;

  let appBuild: number | undefined;
  if (body.app_build != null && body.app_build !== "") {
    const parsed = Number(body.app_build);
    if (Number.isFinite(parsed) && parsed > 0) {
      appBuild = Math.trunc(parsed);
    }
  }

  return {
    hwid,
    os: body.os ? String(body.os) : undefined,
    os_version: body.os_version ? String(body.os_version) : undefined,
    app_version: body.app_version ? String(body.app_version) : undefined,
    app_build: appBuild,
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
      appBuild: device.app_build,
    },
    create: {
      userId,
      hwid: device.hwid,
      os: device.os,
      osVersion: device.os_version,
      appVersion: device.app_version,
      appBuild: device.app_build,
    },
  });
}

export async function createUserDevice(
  userId: string,
  device: DeviceInput,
  opts?: { maxDevices?: number; label?: string | null },
) {
  const record = await upsertUserDevice(userId, device, opts);
  if (opts?.label !== undefined) {
    return prisma.device.update({
      where: { id: record.id },
      data: { label: opts.label },
    });
  }
  return record;
}

export async function deleteUserDevice(deviceId: string) {
  await prisma.refreshToken.deleteMany({ where: { deviceId } });
  return prisma.device.delete({ where: { id: deviceId } });
}

export async function findUserDevice(userId: string, hwid: string) {
  return prisma.device.findFirst({ where: { userId, hwid } });
}
