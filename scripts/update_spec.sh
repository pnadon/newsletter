#!/usr/bin/env bash
set -eo pipefail

if [[ -z "${VAULT_ADDR}" ]]; then
  echo "VAULT_ADDR must be set!"
  exit 1
fi

if ! [ -x "$(command -v vault)" ]; then
  echo >&2 "Error: `vault` is not installed."
  exit 1
fi

if ! [ -x "$(command -v consul-template)" ]; then
  echo >&2 "Error: `consul-template` is not installed."
  exit 1
fi

if ! [ -x "$(command -v doctl)" ]; then
  echo >&2 "Error: `doctl` is not installed."
  exit 1
fi

echo "Retrieving secret from Vault and applying to template"
echo "Ensure you are logged into vault"

consul-template -template "spec.yaml.tmpl:spec.yaml" -once

echo "Success! Updating DigitalOcean with the new spec..."

# If this step fails the spec.yaml file will remain on disk. 
# Not a huge deal since it's ignored by git, and can be viewed for debugging.
doctl apps update $(vault kv get -field=app_id kv/newsletter) --spec=spec.yaml

echo "Success! Removing the spec file"

rm spec.yaml

