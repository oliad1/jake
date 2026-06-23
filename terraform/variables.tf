variable "supabase_org_id" {
  type        = string
}

variable "supabase_db_password" {
  type        = string
  sensitive   = true
}

variable "supabase_access_token" {
  type        = string
  sensitive   = true
}
