-- CreateTable
CREATE TABLE "MinerCredential" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "mac" TEXT NOT NULL,
    "username" TEXT NOT NULL,
    "password_enc" TEXT NOT NULL,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "MinerCredential_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE INDEX "MinerCredential_userId_idx" ON "MinerCredential"("userId");

-- CreateIndex
CREATE UNIQUE INDEX "MinerCredential_userId_mac_key" ON "MinerCredential"("userId", "mac");

-- AddForeignKey
ALTER TABLE "MinerCredential" ADD CONSTRAINT "MinerCredential_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE CASCADE ON UPDATE CASCADE;
