#!/bin/sh
set -e
API_URL="${PUBLIC_API_URL:-https://api.mpulse.bob4.fun}"
sed "s|__PUBLIC_API_URL__|${API_URL}|g" /usr/share/nginx/html/index.html > /tmp/index.html
mv /tmp/index.html /usr/share/nginx/html/index.html
exec nginx -g 'daemon off;'
