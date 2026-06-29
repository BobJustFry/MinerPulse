import { SubscriptionStatus } from "@minerpulse/db";
import { prisma } from "./prisma.js";

export async function activeSubscription(userId: string) {
  const now = new Date();
  return prisma.subscription.findFirst({
    where: {
      userId,
      status: SubscriptionStatus.ACTIVE,
      OR: [{ endsAt: null }, { endsAt: { gt: now } }],
    },
    include: { plan: true },
    orderBy: { createdAt: "desc" },
  });
}
