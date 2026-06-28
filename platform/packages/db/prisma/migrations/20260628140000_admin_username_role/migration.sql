-- Admin login by username + super-admin role

CREATE TYPE "AdminRole" AS ENUM ('SUPER_ADMIN', 'ADMIN');

ALTER TABLE "AdminUser" ADD COLUMN "username" TEXT;
ALTER TABLE "AdminUser" ADD COLUMN "role" "AdminRole" NOT NULL DEFAULT 'ADMIN';

UPDATE "AdminUser" SET "username" = COALESCE(NULLIF(split_part("email", '@', 1), ''), 'admin')
WHERE "username" IS NULL;

ALTER TABLE "AdminUser" ALTER COLUMN "username" SET NOT NULL;
CREATE UNIQUE INDEX "AdminUser_username_key" ON "AdminUser"("username");

ALTER TABLE "AdminUser" ALTER COLUMN "email" DROP NOT NULL;
