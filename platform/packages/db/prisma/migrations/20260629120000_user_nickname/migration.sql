-- AlterTable
ALTER TABLE "User" ADD COLUMN "nickname" TEXT;

UPDATE "User" SET "nickname" = split_part("email", '@', 1) || '_' || substr("id", 1, 4) WHERE "nickname" IS NULL;

ALTER TABLE "User" ALTER COLUMN "nickname" SET NOT NULL;

CREATE UNIQUE INDEX "User_nickname_key" ON "User"("nickname");
