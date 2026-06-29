-- Rename fingerprint to hwid and add OS metadata
ALTER TABLE "Device" RENAME COLUMN "fingerprint" TO "hwid";

ALTER TABLE "Device" ADD COLUMN "os" TEXT;
ALTER TABLE "Device" ADD COLUMN "os_version" TEXT;
ALTER TABLE "Device" ADD COLUMN "app_version" TEXT;

DROP INDEX "Device_userId_fingerprint_key";
CREATE UNIQUE INDEX "Device_userId_hwid_key" ON "Device"("userId", "hwid");
