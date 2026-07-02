-- Per-user device limit override (null = use subscription plan limit)
ALTER TABLE "User" ADD COLUMN "max_devices_override" INTEGER;
