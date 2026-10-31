# Distribution d'Augur : deux buckets R2 servis par Cloudflare sur des
# sous-domaines dédiés.
#
#   updates.<domaine>  installeurs signés et manifestes de mise à jour Tauri
#   data.<domaine>     paquets de données de jeu versionnés par patch
#
# Tout y est immuable sauf les pointeurs de canal (cache 60 s, purgés à la
# bascule). R2 ne facture pas la sortie de trafic, ce qui absorbe le pic de
# téléchargement d'un lancement de ligue.

locals {
  suffix = var.environment == "production" ? "" : "-${var.environment}"

  hosts = {
    updates = "updates${local.suffix}.${var.domain}"
    data    = "data${local.suffix}.${var.domain}"
  }

  all_hosts = flatten([
    for env_suffix in ["", "-staging"] : [
      "updates${env_suffix}.${var.domain}",
      "data${env_suffix}.${var.domain}",
    ]
  ])
}

resource "cloudflare_r2_bucket" "this" {
  for_each = local.hosts

  account_id = var.account_id
  name       = "augur-${each.key}${local.suffix}"
  location   = var.bucket_location
}

resource "cloudflare_r2_custom_domain" "this" {
  for_each = local.hosts

  account_id  = var.account_id
  bucket_name = cloudflare_r2_bucket.this[each.key].name
  domain      = each.value
  zone_id     = var.zone_id
  enabled     = true
  min_tls     = "1.2"
}

# Par défaut, Cloudflare ne met pas en cache les .json ni les .msi : on rend
# tout éligible sur les hôtes de distribution en respectant le Cache-Control
# posé à l'envoi par la CI (immutable pour les versions, 60 s pour les canaux).
#
# Une zone n'accepte qu'un seul ruleset par phase : il est géré uniquement
# depuis l'environnement production et couvre aussi les hôtes de staging.
resource "cloudflare_ruleset" "cache" {
  count = var.environment == "production" ? 1 : 0

  zone_id     = var.zone_id
  name        = "augur-distribution-cache"
  description = "Cache des hôtes de distribution d'Augur"
  kind        = "zone"
  phase       = "http_request_cache_settings"

  rules = [{
    description = "Distribution Augur : cache selon l'origine"
    expression  = "(http.host in {${join(" ", formatlist("\"%s\"", local.all_hosts))}})"
    action      = "set_cache_settings"
    enabled     = true

    action_parameters = {
      cache       = true
      edge_ttl    = { mode = "respect_origin" }
      browser_ttl = { mode = "respect_origin" }
    }
  }]
}
