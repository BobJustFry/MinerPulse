{
  email {{LETSENCRYPT_EMAIL}}
}

{{BASE_DOMAIN}} {
{{HTTP_TO_HTTPS_REDIRECT}}
  reverse_proxy web:3000
}

api.{{BASE_DOMAIN}} {
{{HTTP_TO_HTTPS_REDIRECT}}
  reverse_proxy api:3001
}

admin.{{BASE_DOMAIN}} {
{{HTTP_TO_HTTPS_REDIRECT}}
{{ADMIN_IP_BLOCK}}
}
