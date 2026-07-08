-- Client diagnostic log uploads from desktop app
CREATE TABLE "ClientLog" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "hwid" TEXT NOT NULL,
    "filename" TEXT NOT NULL,
    "size_bytes" INTEGER NOT NULL,
    "storage_path" TEXT NOT NULL,
    "app_version" TEXT,
    "app_build" INTEGER,
    "timezone" TEXT,
    "created_at" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "ClientLog_pkey" PRIMARY KEY ("id")
);

CREATE INDEX "ClientLog_userId_created_at_idx" ON "ClientLog"("userId", "created_at");
CREATE INDEX "ClientLog_hwid_idx" ON "ClientLog"("hwid");

ALTER TABLE "ClientLog" ADD CONSTRAINT "ClientLog_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;
