terraform {
  required_version = ">= 1.8"

  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5.0"
    }
  }

  # État distant : à activer une fois le bucket d'état créé (voir README.md).
  # backend "s3" {
  #   bucket                      = "augur-tofu-state"
  #   key                         = "distribution/terraform.tfstate"
  #   region                      = "auto"
  #   endpoints                   = { s3 = "https://<ACCOUNT_ID>.r2.cloudflarestorage.com" }
  #   skip_credentials_validation = true
  #   skip_region_validation      = true
  #   skip_requesting_account_id  = true
  #   skip_metadata_api_check     = true
  #   skip_s3_checksum            = true
  #   use_path_style              = true
  # }
}

provider "cloudflare" {
  # Jeton lu dans CLOUDFLARE_API_TOKEN (jamais dans le code ni dans les tfvars).
}
