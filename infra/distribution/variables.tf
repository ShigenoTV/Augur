variable "environment" {
  description = "Environnement : staging ou production."
  type        = string

  validation {
    condition     = contains(["staging", "production"], var.environment)
    error_message = "environment doit valoir staging ou production."
  }
}

variable "account_id" {
  description = "Identifiant du compte Cloudflare."
  type        = string
}

variable "zone_id" {
  description = "Identifiant de la zone DNS Cloudflare du domaine d'Augur."
  type        = string
}

variable "domain" {
  description = "Domaine racine (ex. augur.example). Provisoire tant que Max n'a pas choisi le domaine."
  type        = string
}

variable "bucket_location" {
  description = "Indice de localisation R2 (weur = Europe de l'Ouest)."
  type        = string
  default     = "weur"
}
