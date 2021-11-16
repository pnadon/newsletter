# Run `. ./scripts/set_env.sh` for a convenient way to set envs
export APP_EMAIL_CLIENT__AUTHORIZATION_TOKEN=$(vault kv get -field=authorization_token kv/newsletter)
export APP_EMAIL_CLIENT__SENDER_EMAIL=$(vault kv get -field=sender_email kv/newsletter)
