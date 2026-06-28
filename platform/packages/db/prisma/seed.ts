import { AdminRole, PrismaClient, Tier } from "@prisma/client";
import bcrypt from "bcryptjs";

const prisma = new PrismaClient();

const DEFAULT_SUPER_ADMIN_USERNAME = "mpulse-admin";

async function main() {
  const username = process.env.BOOTSTRAP_ADMIN_USERNAME?.trim() || DEFAULT_SUPER_ADMIN_USERNAME;
  const adminPassword = process.env.BOOTSTRAP_ADMIN_PASSWORD;

  if (adminPassword) {
    const passwordHash = await bcrypt.hash(adminPassword, 12);
    await prisma.adminUser.upsert({
      where: { username },
      update: { passwordHash, role: AdminRole.SUPER_ADMIN },
      create: { username, passwordHash, role: AdminRole.SUPER_ADMIN },
    });
    console.log(`Super admin ready: ${username}`);
  } else {
    console.warn("BOOTSTRAP_ADMIN_PASSWORD not set — super admin not created");
  }

  const plans = [
    {
      slug: "client-monthly",
      name: "Client — 1 month",
      tier: Tier.CLIENT,
      priceCents: 99000,
      durationDays: 30,
      maxDevices: 1,
      sortOrder: 10,
    },
    {
      slug: "service-monthly",
      name: "Service — 1 month",
      tier: Tier.SERVICE,
      priceCents: 199000,
      durationDays: 30,
      maxDevices: 2,
      sortOrder: 20,
    },
  ];

  for (const plan of plans) {
    await prisma.plan.upsert({
      where: { slug: plan.slug },
      update: plan,
      create: plan,
    });
  }

  console.log("Default plans seeded");
}

main()
  .catch((err) => {
    console.error(err);
    process.exit(1);
  })
  .finally(async () => {
    await prisma.$disconnect();
  });
