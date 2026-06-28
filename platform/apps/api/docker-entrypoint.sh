#!/bin/sh
set -e
cd /app/packages/db
npx prisma migrate deploy
npx tsx prisma/seed.ts || true
cd /app/apps/api
exec node dist/index.js
