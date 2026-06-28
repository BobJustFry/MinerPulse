FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json* ./
COPY packages/db/package.json packages/db/
COPY apps/api/package.json apps/api/
RUN npm install --workspace=@minerpulse/db --workspace=@minerpulse/api
COPY packages/db packages/db
COPY apps/api apps/api
RUN npm run generate -w @minerpulse/db
RUN npm run build -w @minerpulse/api

FROM node:22-alpine
WORKDIR /app
ENV NODE_ENV=production
COPY package.json package-lock.json* ./
COPY packages/db/package.json packages/db/
COPY apps/api/package.json apps/api/
RUN npm install --omit=dev --workspace=@minerpulse/db --workspace=@minerpulse/api
COPY --from=build /app/packages/db packages/db
COPY --from=build /app/apps/api/dist apps/api/dist
COPY packages/db/prisma packages/db/prisma
COPY apps/api/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
EXPOSE 3001
ENTRYPOINT ["/entrypoint.sh"]
