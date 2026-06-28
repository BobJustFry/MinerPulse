FROM nginx:alpine
COPY public /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY docker-entrypoint.sh /docker-entrypoint.sh
RUN chmod +x /docker-entrypoint.sh
ENV PUBLIC_API_URL=https://api.mpulse.bob4.fun
EXPOSE 3000
ENTRYPOINT ["/docker-entrypoint.sh"]
