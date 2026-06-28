import prismaPkg from "@prisma/client";

export const PrismaClient = prismaPkg.PrismaClient;
export const Tier = prismaPkg.Tier;
export const SubscriptionStatus = prismaPkg.SubscriptionStatus;
export const PaymentStatus = prismaPkg.PaymentStatus;
export const AdminRole = prismaPkg.AdminRole;

export type Tier = (typeof prismaPkg.Tier)[keyof typeof prismaPkg.Tier];
export type SubscriptionStatus = (typeof prismaPkg.SubscriptionStatus)[keyof typeof prismaPkg.SubscriptionStatus];
export type PaymentStatus = (typeof prismaPkg.PaymentStatus)[keyof typeof prismaPkg.PaymentStatus];
export type AdminRole = (typeof prismaPkg.AdminRole)[keyof typeof prismaPkg.AdminRole];

export type {
  User,
  AdminUser,
  Plan,
  Subscription,
  Device,
  ActivationCode,
  AuditLog,
} from "@prisma/client";
