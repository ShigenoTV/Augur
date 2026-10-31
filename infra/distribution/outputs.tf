output "updates_base_url" {
  description = "À reporter dans la variable GitHub AUGUR_UPDATES_BASE_URL."
  value       = "https://${local.hosts.updates}"
}

output "data_base_url" {
  description = "À reporter dans la variable GitHub AUGUR_DATA_BASE_URL."
  value       = "https://${local.hosts.data}"
}

output "buckets" {
  description = "Noms des buckets (variables R2_UPDATES_BUCKET et R2_DATA_BUCKET)."
  value       = { for k, b in cloudflare_r2_bucket.this : k => b.name }
}
