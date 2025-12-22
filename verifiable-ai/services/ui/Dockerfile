# build stage
FROM node:20-alpine AS build
WORKDIR /app

COPY package*.json ./
RUN npm ci

COPY . .
# Build-time API URL (baked into bundle)
ARG VITE_API_URL=http://localhost:8080
ENV VITE_API_URL=$VITE_API_URL

RUN npm run build

# run stage
FROM nginx:alpine
COPY --from=build /app/dist /usr/share/nginx/html

# Optional: SPA fallback so refresh on /job/123 doesn't 404
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
