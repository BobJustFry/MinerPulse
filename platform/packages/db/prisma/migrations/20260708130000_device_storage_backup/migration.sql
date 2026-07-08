-- Per-HWID credential storage backup + shared/isolated storage preference
ALTER TABLE "User" ADD COLUMN "shared_storage" BOOLEAN NOT NULL DEFAULT true;

CREATE TABLE "DeviceStorageBackup" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "hwid" TEXT NOT NULL,
    "payload_enc" TEXT NOT NULL,
    "updated_at" TIMESTAMP(3) NOT NULL,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "DeviceStorageBackup_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX "DeviceStorageBackup_userId_hwid_key" ON "DeviceStorageBackup"("userId", "hwid");
CREATE INDEX "DeviceStorageBackup_userId_idx" ON "DeviceStorageBackup"("userId");

ALTER TABLE "DeviceStorageBackup" ADD CONSTRAINT "DeviceStorageBackup_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;
